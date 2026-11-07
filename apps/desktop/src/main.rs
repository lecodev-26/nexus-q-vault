#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nexus_q_vault_spike::folder_manifest::{
    decrypt_folder as decrypt_folder_from_vault, encrypt_folder as encrypt_folder_to_vault,
};
use nexusq_core::{
    storage::backup,
    vault::{Algorithm, KeyId, KeyStatus, Purpose, Session, Vault},
};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use zeroize::Zeroizing;

struct AppState {
    session: Mutex<Option<Session>>,
    last_activity: Mutex<Instant>,
    idle_timeout: Mutex<Duration>,
    watchdog_stop: AtomicBool,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            last_activity: Mutex::new(Instant::now()),
            idle_timeout: Mutex::new(Duration::from_secs(300)),
            watchdog_stop: AtomicBool::new(false),
        }
    }
}
fn idle_timeout_from_seconds(seconds: u64) -> Result<Duration, String> {
    if ![60, 300, 600, 900, 1800].contains(&seconds) {
        return Err("El tiempo de inactividad seleccionado no es válido".into());
    }
    Ok(Duration::from_secs(seconds))
}

fn inactivity_elapsed(last_activity: Instant, now: Instant, timeout: Duration) -> bool {
    !timeout.is_zero() && now.duration_since(last_activity) >= timeout
}

fn touch_activity(state: &AppState) -> Result<(), String> {
    *state
        .last_activity
        .lock()
        .map_err(|_| "No se pudo actualizar la actividad".to_string())? = Instant::now();
    Ok(())
}
fn lock_session(state: &AppState) -> Result<bool, String> {
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    let Some(session) = guard.take() else {
        return Ok(false);
    };
    session
        .lock()
        .map_err(|error| format!("No se pudo guardar y bloquear el vault: {error}"))?;
    drop(guard);
    Ok(true)
}
fn auto_lock_if_idle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let timeout = match state.idle_timeout.lock() {
        Ok(value) => *value,
        Err(_) => return,
    };
    let last_activity = match state.last_activity.lock() {
        Ok(value) => *value,
        Err(_) => return,
    };
    if !inactivity_elapsed(last_activity, Instant::now(), timeout) {
        return;
    }
    let mut session = match state.session.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    let still_idle = state
        .last_activity
        .lock()
        .map(|last| inactivity_elapsed(*last, Instant::now(), timeout))
        .unwrap_or(false);
    if !still_idle || session.is_none() {
        return;
    }
    let Some(active) = session.take() else {
        return;
    };
    let lock_result = active.lock();
    drop(session);
    match lock_result {
        Ok(_) => {
            let _ = app.emit("vault-auto-locked", "inactivity");
        }
        Err(error) => {
            let _ = app.emit(
                "vault-lock-failed",
                format!("No se pudo guardar y bloquear automáticamente el vault: {error}"),
            );
        }
    }
}

fn secret_bytes(value: String) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(value.into_bytes())
}

fn active_session<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Option<Session>>, String> {
    let guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    if guard.is_none() {
        return Err("Primero debes desbloquear un vault".into());
    }
    Ok(guard)
}

fn active_file_key(session: &Session) -> Result<KeyId, String> {
    session
        .list_keys()
        .find(|record| record.purpose() == Purpose::Encrypt && record.status() == KeyStatus::Active)
        .map(|record| record.key_id().clone())
        .ok_or_else(|| "El vault no tiene una clave activa para cifrar".to_string())
}

#[tauri::command]
fn vault_status(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    Ok(guard.is_some())
}
#[tauri::command]
fn record_activity(state: State<'_, AppState>) -> Result<(), String> {
    let _session = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    touch_activity(&state)
}
#[tauri::command]
fn set_idle_timeout(state: State<'_, AppState>, seconds: u64) -> Result<(), String> {
    let timeout = idle_timeout_from_seconds(seconds)?;
    let _session = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    *state
        .idle_timeout
        .lock()
        .map_err(|_| "No se pudo guardar el tiempo de bloqueo".to_string())? = timeout;
    touch_activity(&state)
}
#[tauri::command]
fn create_vault(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err("Indica dónde guardar el vault".into());
    }
    if password.len() < 12 {
        return Err("Usa una contraseña de al menos 12 bytes".into());
    }
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    if guard.is_some() {
        return Err("Bloquea el vault actual antes de crear otro".into());
    }

    let password = secret_bytes(password);
    let vault = Vault::create(&path, password.as_slice(), Some("NEXUS-Q Vault".into()))
        .map_err(|e| format!("No se pudo crear el vault: {e}"))?;
    let mut session = vault
        .unlock(password.as_slice())
        .map_err(|e| format!("No se pudo inicializar el vault: {e}"))?;
    let key_id = session
        .generate_key(Algorithm::Aes256Gcm, Purpose::Encrypt)
        .map_err(|e| format!("No se pudo preparar la clave de archivos: {e}"))?;
    session
        .activate_key(&key_id)
        .map_err(|e| format!("No se pudo activar la clave de archivos: {e}"))?;
    let vault = session
        .lock()
        .map_err(|e| format!("No se pudo guardar el vault: {e}"))?;
    let session = vault
        .unlock(password.as_slice())
        .map_err(|e| format!("No se pudo verificar el vault creado: {e}"))?;
    *guard = Some(session);
    touch_activity(&state)?;
    Ok("Vault creado, verificado y desbloqueado".into())
}

#[tauri::command]
fn open_vault(
    state: State<'_, AppState>,
    path: String,
    password: String,
) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err("Indica la ruta del vault".into());
    }
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    if guard.is_some() {
        return Err("Bloquea el vault actual antes de abrir otro".into());
    }
    let password = secret_bytes(password);
    let session = Vault::open(&path)
        .and_then(|vault| vault.unlock(password.as_slice()))
        .map_err(|e| format!("No se pudo desbloquear el vault: {e}"))?;
    *guard = Some(session);
    touch_activity(&state)?;
    Ok("Vault desbloqueado".into())
}

#[tauri::command]
fn lock_vault(state: State<'_, AppState>) -> Result<String, String> {
    if lock_session(&state)? {
        Ok("Vault guardado y bloqueado".into())
    } else {
        Err("No hay ningún vault desbloqueado".into())
    }
}

#[tauri::command]
fn encrypt_file(state: State<'_, AppState>, input_path: String) -> Result<String, String> {
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let key_id = session
        .list_keys()
        .find(|record| record.purpose() == Purpose::Encrypt && record.status() == KeyStatus::Active)
        .map(|record| record.key_id().clone())
        .ok_or_else(|| "El vault no tiene una clave activa para cifrar".to_string())?;
    let output = session
        .encrypt_file(&input_path, &key_id, Vec::new())
        .map_err(|e| format!("No se pudo cifrar el archivo: {e}"))?;
    Ok(format!("Archivo cifrado: {}", output.display()))
}

#[tauri::command]
fn decrypt_file(
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
) -> Result<String, String> {
    if output_path.trim().is_empty() {
        return Err("Indica dónde restaurar el archivo".into());
    }
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    session
        .decrypt_file(&input_path, &output_path)
        .map_err(|e| format!("No se pudo restaurar el archivo: {e}"))?;
    Ok(format!("Archivo restaurado: {output_path}"))
}

#[tauri::command]
fn encrypt_folder(
    state: State<'_, AppState>,
    source_path: String,
    vault_folder_path: String,
) -> Result<String, String> {
    if source_path.trim().is_empty() || vault_folder_path.trim().is_empty() {
        return Err("Indica la carpeta original y dónde guardar el vault de carpeta".into());
    }
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let key_id = active_file_key(session)?;
    let count = encrypt_folder_to_vault(
        session,
        &key_id,
        std::path::Path::new(&source_path),
        std::path::Path::new(&vault_folder_path),
    )
    .map_err(|error| format!("No se pudo cifrar la carpeta: {error}"))?;
    Ok(format!("Carpeta cifrada: {count} archivos protegidos"))
}

#[tauri::command]
fn decrypt_folder(
    state: State<'_, AppState>,
    vault_folder_path: String,
    destination_path: String,
) -> Result<String, String> {
    if vault_folder_path.trim().is_empty() || destination_path.trim().is_empty() {
        return Err("Indica la carpeta vault y el destino de restauración".into());
    }
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let count = decrypt_folder_from_vault(
        session,
        std::path::Path::new(&vault_folder_path),
        std::path::Path::new(&destination_path),
    )
    .map_err(|error| format!("No se pudo restaurar la carpeta: {error}"))?;
    Ok(format!("Carpeta restaurada: {count} archivos recuperados"))
}

#[tauri::command]
fn export_backup(
    state: State<'_, AppState>,
    backup_path: String,
    backup_passphrase: String,
) -> Result<String, String> {
    if backup_path.trim().is_empty() {
        return Err("Indica dónde guardar el backup".into());
    }
    if backup_passphrase.len() < 12 {
        return Err("Usa una contraseña de backup de al menos 12 bytes".into());
    }
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let passphrase = secret_bytes(backup_passphrase);
    backup::export(session, &backup_path, passphrase.as_slice())
        .map_err(|e| format!("No se pudo crear el backup: {e}"))?;
    Ok("Backup cifrado creado".into())
}

#[tauri::command]
fn import_backup(
    state: State<'_, AppState>,
    backup_path: String,
    backup_passphrase: String,
    new_vault_path: String,
    new_vault_password: String,
) -> Result<String, String> {
    if backup_path.trim().is_empty() || new_vault_path.trim().is_empty() {
        return Err("Indica la ruta del backup y la del nuevo vault".into());
    }
    if backup_passphrase.len() < 12 {
        return Err("Usa la contraseña independiente del backup (mínimo 12 bytes)".into());
    }
    if new_vault_password.len() < 12 {
        return Err("Usa una contraseña nueva de al menos 12 bytes".into());
    }
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    if guard.is_some() {
        return Err("Bloquea el vault actual antes de importar un backup".into());
    }
    let passphrase = secret_bytes(backup_passphrase);
    let password = secret_bytes(new_vault_password);
    let vault = backup::import(
        &backup_path,
        passphrase.as_slice(),
        &new_vault_path,
        password.as_slice(),
    )
    .map_err(|e| format!("No se pudo restaurar el backup: {e}"))?;
    let session = vault
        .unlock(password.as_slice())
        .map_err(|e| format!("No se pudo verificar el vault restaurado: {e}"))?;
    *guard = Some(session);
    touch_activity(&state)?;
    Ok("Backup restaurado en un nuevo vault y verificado".into())
}

fn selected_path(path: Option<FilePath>) -> Result<Option<String>, String> {
    match path {
        Some(path) => {
            let path = path
                .into_path()
                .map_err(|error| format!("No se pudo interpretar la ruta seleccionada: {error}"))?;
            let path = path
                .to_str()
                .ok_or_else(|| "La ruta seleccionada no es compatible con UTF-8".to_string())?;
            Ok(Some(path.to_owned()))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn pick_file_path(app: AppHandle, kind: String) -> Result<Option<String>, String> {
    let selected = match kind.as_str() {
        "vault" => app
            .dialog()
            .file()
            .set_title("Seleccionar vault NEXUS-Q")
            .add_filter("Vault NEXUS-Q", &["nqv"])
            .blocking_pick_file(),
        "backup" => app
            .dialog()
            .file()
            .set_title("Seleccionar backup cifrado")
            .add_filter("Backup NEXUS-Q", &["nqb"])
            .blocking_pick_file(),
        "encrypted" => app
            .dialog()
            .file()
            .set_title("Seleccionar archivo cifrado")
            .add_filter("Archivo NEXUS-Q", &["nqx"])
            .blocking_pick_file(),
        "any" => app
            .dialog()
            .file()
            .set_title("Seleccionar archivo original")
            .blocking_pick_file(),
        _ => return Err("Tipo de selector de archivo no válido".into()),
    };
    selected_path(selected)
}

#[tauri::command]
async fn pick_save_path(app: AppHandle, kind: String) -> Result<Option<String>, String> {
    let selected = match kind.as_str() {
        "vault" => app
            .dialog()
            .file()
            .set_title("Guardar vault NEXUS-Q")
            .add_filter("Vault NEXUS-Q", &["nqv"])
            .set_file_name("nexus-q-vault.nqv")
            .blocking_save_file(),
        "backup" => app
            .dialog()
            .file()
            .set_title("Guardar backup cifrado")
            .add_filter("Backup NEXUS-Q", &["nqb"])
            .set_file_name("backup.nqb")
            .blocking_save_file(),
        "restored" => app
            .dialog()
            .file()
            .set_title("Guardar archivo restaurado")
            .set_file_name("restored-file")
            .blocking_save_file(),
        _ => return Err("Tipo de destino no válido".into()),
    };
    selected_path(selected)
}

#[tauri::command]
async fn pick_folder_path(app: AppHandle, kind: String) -> Result<Option<String>, String> {
    let title = match kind.as_str() {
        "source" => "Seleccionar carpeta original",
        "vault-folder" => "Seleccionar carpeta vault",
        "parent" => "Seleccionar carpeta contenedora",
        _ => return Err("Tipo de selector de carpeta no válido".into()),
    };
    let selected = app.dialog().file().set_title(title).blocking_pick_folder();
    selected_path(selected)
}

#[tauri::command]
fn open_project_repository() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", "https://github.com/lecodev-26/nexus-q"])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("https://github.com/lecodev-26/nexus-q")
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg("https://github.com/lecodev-26/nexus-q")
        .spawn();
    result
        .map(|_| ())
        .map_err(|error| format!("No se pudo abrir GitHub: {error}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    let should_stop = {
                        let state = handle.state::<AppState>();
                        state.watchdog_stop.load(Ordering::Relaxed)
                    };
                    if should_stop {
                        break;
                    }
                    auto_lock_if_idle(&handle);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_status,
            record_activity,
            set_idle_timeout,
            create_vault,
            open_vault,
            lock_vault,
            encrypt_file,
            decrypt_file,
            encrypt_folder,
            decrypt_folder,
            export_backup,
            import_backup,
            pick_file_path,
            pick_save_path,
            pick_folder_path,
            open_project_repository
        ])
        .build(tauri::generate_context!())
        .expect("error while building NEXUS-Q Vault")
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                let state = app.state::<AppState>();
                if let Err(error) = lock_session(&state) {
                    let _ = app.emit("vault-lock-failed", error);
                    api.prevent_exit();
                }
            }
            RunEvent::Resumed => {
                let state = app.state::<AppState>();
                match lock_session(&state) {
                    Ok(true) => {
                        let _ = app.emit("vault-auto-locked", "system-resume");
                    }
                    Ok(false) => {}
                    Err(error) => {
                        let _ = app.emit("vault-lock-failed", error);
                    }
                }
                let _ = touch_activity(&state);
            }
            RunEvent::Exit => app
                .state::<AppState>()
                .watchdog_stop
                .store(true, Ordering::Relaxed),
            _ => {}
        });
}

#[cfg(test)]
mod idle_lock_tests {
    use super::{idle_timeout_from_seconds, inactivity_elapsed};
    use std::time::{Duration, Instant};

    #[test]
    fn accepts_only_supported_idle_timeouts() {
        for seconds in [60, 300, 600, 900, 1800] {
            assert_eq!(
                idle_timeout_from_seconds(seconds),
                Ok(Duration::from_secs(seconds))
            );
        }
        for seconds in [0, 30, 120, 3600] {
            assert!(idle_timeout_from_seconds(seconds).is_err());
        }
    }

    #[test]
    fn inactivity_deadline_is_inclusive_and_disabled_for_zero() {
        let now = Instant::now();
        let last_activity = now - Duration::from_secs(300);
        assert!(!inactivity_elapsed(
            last_activity,
            now,
            Duration::from_secs(301)
        ));
        assert!(inactivity_elapsed(
            last_activity,
            now,
            Duration::from_secs(300)
        ));
        assert!(!inactivity_elapsed(last_activity, now, Duration::ZERO));
    }
}
