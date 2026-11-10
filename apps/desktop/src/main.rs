#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod sharing;

use nexusq_core::identity::IdentityId;
use nexusq_core::{
    storage::backup,
    vault::{
        Algorithm, DestructionConfirmation, KeyId, KeyStatus, OperationControl, OperationPhase,
        OperationProgress, Purpose, Session, Vault,
    },
};
use std::{
    collections::HashMap,
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
    auto_lock_retry_after: Mutex<Option<Instant>>,
    watchdog_stop: AtomicBool,
    operations: Mutex<HashMap<String, OperationControl>>,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            last_activity: Mutex::new(Instant::now()),
            idle_timeout: Mutex::new(Duration::from_secs(300)),
            auto_lock_retry_after: Mutex::new(None),
            watchdog_stop: AtomicBool::new(false),
            operations: Mutex::new(HashMap::new()),
        }
    }
}
const AUTO_LOCK_RETRY_DELAY: Duration = Duration::from_secs(30);

fn idle_timeout_from_seconds(seconds: u64) -> Result<Duration, String> {
    if seconds != 0 && ![60, 300, 600, 900, 1800].contains(&seconds) {
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
    *state
        .auto_lock_retry_after
        .lock()
        .map_err(|_| "No se pudo actualizar el estado de bloqueo".to_string())? = None;
    Ok(())
}

fn defer_auto_lock_retry(state: &AppState) {
    if let Ok(mut retry_after) = state.auto_lock_retry_after.lock() {
        *retry_after = Some(Instant::now() + AUTO_LOCK_RETRY_DELAY);
    }
}
fn lock_session_slot(slot: &mut Option<Session>) -> Result<bool, String> {
    let Some(session) = slot.take() else {
        return Ok(false);
    };
    match session.lock_recoverable() {
        Ok(_) => Ok(true),
        Err(failure) => {
            let (session, error) = failure.into_parts();
            *slot = Some(session);
            Err(format!("No se pudo guardar y bloquear el vault: {error}"))
        }
    }
}

fn lock_session(state: &AppState) -> Result<bool, String> {
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    let result = lock_session_slot(&mut guard);
    if result.is_err() {
        defer_auto_lock_retry(state);
    }
    result
}

fn resume_session(state: &AppState) -> Result<bool, String> {
    // A failed resume lock must retain the old inactivity deadline and retry
    // throttle; treating resume as fresh activity would postpone protection.
    let locked = lock_session(state)?;
    touch_activity(state)?;
    Ok(locked)
}
#[derive(Debug, PartialEq, Eq)]
enum AutoLockOutcome {
    NotDue,
    Locked,
    Failed(String),
}

fn auto_lock_state(state: &AppState) -> AutoLockOutcome {
    let now = Instant::now();
    let retry_after = match state.auto_lock_retry_after.lock() {
        Ok(value) => *value,
        Err(_) => return AutoLockOutcome::NotDue,
    };
    if retry_after.is_some_and(|deadline| now < deadline) {
        return AutoLockOutcome::NotDue;
    }

    let timeout = match state.idle_timeout.lock() {
        Ok(value) => *value,
        Err(_) => return AutoLockOutcome::NotDue,
    };
    let last_activity = match state.last_activity.lock() {
        Ok(value) => *value,
        Err(_) => return AutoLockOutcome::NotDue,
    };
    if !inactivity_elapsed(last_activity, now, timeout) {
        return AutoLockOutcome::NotDue;
    }

    let mut session = match state.session.lock() {
        Ok(value) => value,
        Err(_) => return AutoLockOutcome::NotDue,
    };
    let still_idle = state
        .last_activity
        .lock()
        .map(|last| inactivity_elapsed(*last, Instant::now(), timeout))
        .unwrap_or(false);
    if !still_idle || session.is_none() {
        return AutoLockOutcome::NotDue;
    }

    match lock_session_slot(&mut session) {
        Ok(true) => {
            if let Ok(mut retry_after) = state.auto_lock_retry_after.lock() {
                *retry_after = None;
            }
            AutoLockOutcome::Locked
        }
        Ok(false) => AutoLockOutcome::NotDue,
        Err(error) => {
            defer_auto_lock_retry(state);
            AutoLockOutcome::Failed(error)
        }
    }
}

fn auto_lock_if_idle(app: &AppHandle) {
    let state = app.state::<AppState>();
    match auto_lock_state(&state) {
        AutoLockOutcome::NotDue => {}
        AutoLockOutcome::Locked => {
            let _ = app.emit("vault-auto-locked", "inactivity");
        }
        AutoLockOutcome::Failed(error) => {
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

fn confirmed_secret(
    value: String,
    confirmation: String,
    field_name: &str,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let value = secret_bytes(value);
    let confirmation = secret_bytes(confirmation);
    if value.is_empty() {
        return Err(format!("{field_name} no puede estar vacía"));
    }
    if value.as_slice() != confirmation.as_slice() {
        return Err(format!("{field_name} no coincide con su confirmación"));
    }
    Ok(value)
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

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationEvent {
    operation_id: String,
    status: String,
    phase: Option<OperationPhase>,
    completed: Option<u64>,
    total: Option<u64>,
    message: Option<String>,
}

fn emit_operation_event(
    app: &AppHandle,
    operation_id: &str,
    status: &str,
    progress: Option<OperationProgress>,
    message: Option<String>,
) {
    let event = OperationEvent {
        operation_id: operation_id.to_owned(),
        status: status.to_owned(),
        phase: progress.map(|item| item.phase),
        completed: progress.and_then(|item| item.completed),
        total: progress.and_then(|item| item.total),
        message,
    };
    let _ = app.emit("operation-update", event);
}

fn with_operation<T>(
    app: &AppHandle,
    state: &AppState,
    operation_id: &str,
    work: impl FnOnce(&OperationControl) -> Result<T, String>,
) -> Result<T, String> {
    if operation_id.is_empty()
        || operation_id.len() > 64
        || !operation_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Identificador de operación no válido".into());
    }
    let control = OperationControl::new();
    {
        let mut operations = state
            .operations
            .lock()
            .map_err(|_| "No se pudo acceder al centro de operaciones".to_string())?;
        if operations.contains_key(operation_id) {
            return Err("El identificador de operación ya está en uso".into());
        }
        operations.insert(operation_id.to_owned(), control.clone());
    }
    emit_operation_event(app, operation_id, "queued", None, None);
    let result = work(&control);
    let (status, message) = match &result {
        Ok(_) => ("completed", None),
        Err(error)
            if (error.contains("operation cancelled") || error.contains("operación cancelada"))
                && !error.contains("cleanup failed") =>
        {
            (
                "cancelled",
                Some("Cancellation completed at a safe checkpoint; incomplete output was removed or never published.".to_string()),
            )
        }
        Err(_) => (
            "failed",
            Some("Operation failed. Cleanup was attempted where needed; see the form feedback for details.".to_string()),
        ),
    };
    // Error strings can contain OS paths; they are returned to the invoking
    // form only and are deliberately excluded from operation events.
    if let Ok(mut operations) = state.operations.lock() {
        operations.remove(operation_id);
    }
    emit_operation_event(app, operation_id, status, None, message);
    result
}

#[tauri::command]
fn cancel_operation(
    app: AppHandle,
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<bool, String> {
    let operations = state
        .operations
        .lock()
        .map_err(|_| "No se pudo acceder al centro de operaciones".to_string())?;
    if let Some(control) = operations.get(&operation_id) {
        control.cancel();
        emit_operation_event(&app, &operation_id, "cancelling", None, None);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn vault_status(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state
        .session
        .lock()
        .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
    Ok(guard.is_some())
}
fn record_activity_state(state: &AppState) -> Result<(), String> {
    // Activity must remain observable while a long-running operation holds the
    // session mutex; auto_lock_state rechecks this timestamp before locking.
    touch_activity(state)
}

#[tauri::command]
fn record_activity(state: State<'_, AppState>) -> Result<(), String> {
    record_activity_state(&state)
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
    password_confirmation: String,
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

    let password = confirmed_secret(password, password_confirmation, "La contraseña")?;
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
async fn encrypt_file(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    operation_id: String,
) -> Result<String, String> {
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let key_id = session
            .list_keys()
            .find(|record| {
                record.purpose() == Purpose::Encrypt && record.status() == KeyStatus::Active
            })
            .map(|record| record.key_id().clone())
            .ok_or_else(|| "El vault no tiene una clave activa para cifrar".to_string())?;
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        let output = session
            .encrypt_file_controlled(&input_path, &key_id, Vec::new(), control, &mut reporter)
            .map_err(|error| format!("No se pudo cifrar el archivo: {error}"))?;
        Ok(format!("Archivo cifrado: {}", output.display()))
    })
}

#[tauri::command]
async fn decrypt_file(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
    operation_id: String,
) -> Result<String, String> {
    if output_path.trim().is_empty() {
        return Err("Indica dónde restaurar el archivo".into());
    }
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        session
            .decrypt_file_controlled(&input_path, &output_path, control, &mut reporter)
            .map_err(|error| format!("No se pudo restaurar el archivo: {error}"))?;
        Ok(format!("Archivo restaurado: {output_path}"))
    })
}

#[derive(Clone, serde::Serialize)]
struct ReceivingIdentitySummary {
    identity_id: String,
    label: String,
    key_id: String,
    status: String,
    fingerprint: String,
}

fn contact_card_for_identity(
    session: &Session,
    identity_id: &IdentityId,
) -> Result<sharing::ContactCard, String> {
    let identity = session
        .find_identity(identity_id)
        .ok_or_else(|| "The receiving identity no longer exists in this vault".to_string())?;
    if !identity.is_active() {
        return Err("This identity is not active and cannot receive new shared files".into());
    }
    let key_id = identity
        .key_agreement_key
        .as_ref()
        .ok_or_else(|| "This identity has no receiving key configured".to_string())?;
    let key = session
        .find_key(key_id)
        .ok_or_else(|| "The receiving key is missing from this vault".to_string())?;
    if key.metadata.owner.as_deref() != Some(identity_id.as_str())
        || key.algorithm() != Algorithm::MlKem768
        || key.purpose() != Purpose::KeyAgreement
        || key.status() != KeyStatus::Active
    {
        return Err("The receiving key is not active or is not a supported ML-KEM-768 key".into());
    }
    let public_key = key
        .public_key_bytes()
        .ok_or_else(|| "The receiving key has no public component".to_string())?
        .to_vec();
    sharing::ContactCard::new(
        identity_id.to_string(),
        identity
            .metadata
            .label
            .clone()
            .unwrap_or_else(|| "NEXUS-Q recipient".into()),
        key_id.to_string(),
        public_key,
    )
}

fn create_receiving_identity_records_with(
    session: &mut Session,
    label: String,
    create_identity: impl FnOnce(
        &mut Session,
        String,
    ) -> Result<IdentityId, nexusq_core::vault::VaultError>,
    destroy_key: impl FnOnce(&mut Session, &KeyId) -> Result<(), nexusq_core::vault::VaultError>,
) -> Result<(IdentityId, KeyId), String> {
    let key_id = session
        .generate_key(Algorithm::MlKem768, Purpose::KeyAgreement)
        .map_err(|error| format!("Could not generate the receiving key: {error}"))?;
    let identity_id = match create_identity(session, label) {
        Ok(identity_id) => identity_id,
        Err(error) => {
            return match destroy_key(session, &key_id) {
                Ok(()) => Err(format!(
                    "Could not create the receiving identity: {error}. The generated key {key_id} was destroyed."
                )),
                Err(cleanup_error) => Err(format!(
                    "Could not create the receiving identity: {error}. Best-effort cleanup of generated key {key_id} also failed: {cleanup_error}. Cleanup could not be confirmed; inspect key {key_id} after reopening the vault, because an audit-write failure can leave its in-memory status changed without persisting it. If it is still generated, retry Core's explicit key-destruction operation after resolving the policy/audit issue."
                )),
            };
        }
    };
    {
        let key = session
            .find_key_mut(&key_id)
            .map_err(|error| format!("Could not bind the receiving key to its identity: {error}"))?
            .ok_or_else(|| "The newly generated receiving key could not be found".to_string())?;
        key.metadata.owner = Some(identity_id.to_string());
    }
    {
        let identity = session
            .find_identity_mut(&identity_id)
            .map_err(|error| format!("Could not update the receiving identity: {error}"))?
            .ok_or_else(|| "The new receiving identity could not be found".to_string())?;
        identity.key_agreement_key = Some(key_id.clone());
        identity
            .validate()
            .map_err(|error| format!("The receiving identity is invalid: {error}"))?;
    }
    session
        .activate_key(&key_id)
        .map_err(|error| format!("Could not activate the receiving key: {error}"))?;
    Ok((identity_id, key_id))
}

#[tauri::command]
fn create_receiving_identity(
    state: State<'_, AppState>,
    label: String,
) -> Result<sharing::ContactCard, String> {
    let label = label.trim().to_string();
    if label.is_empty() || label.len() > 80 || label.chars().any(char::is_control) {
        return Err("Use a display name between 1 and 80 characters".into());
    }
    let mut guard = active_session(&state)?;
    let session = guard.as_mut().expect("active_session checked");
    let (identity_id, _key_id) = create_receiving_identity_records_with(
        session,
        label,
        |session, label| session.create_identity(Some(label)),
        |session, key_id| session.destroy_key(key_id, DestructionConfirmation::Explicit),
    )?;
    let card = contact_card_for_identity(session, &identity_id)?;
    touch_activity(&state)?;
    Ok(card)
}

#[tauri::command]
fn list_receiving_identities(
    state: State<'_, AppState>,
) -> Result<Vec<ReceivingIdentitySummary>, String> {
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let mut identities = Vec::new();
    for identity in session
        .list_identities()
        .filter(|identity| identity.is_active())
    {
        for key in session.list_keys().filter(|key| {
            key.metadata.owner.as_deref() == Some(identity.id.as_str())
                && key.algorithm() == Algorithm::MlKem768
                && key.purpose() == Purpose::KeyAgreement
                && matches!(
                    key.status(),
                    KeyStatus::Active | KeyStatus::Rotating | KeyStatus::Retired
                )
        }) {
            let Some(public_key) = key.public_key_bytes() else {
                continue;
            };
            let card = sharing::ContactCard::new(
                identity.id.to_string(),
                identity
                    .metadata
                    .label
                    .clone()
                    .unwrap_or_else(|| "NEXUS-Q recipient".into()),
                key.key_id().to_string(),
                public_key.to_vec(),
            )?;
            identities.push(ReceivingIdentitySummary {
                identity_id: card.identity_id,
                label: card.label,
                key_id: card.key_id,
                status: key.status().as_str().to_string(),
                fingerprint: card.fingerprint,
            });
        }
    }
    Ok(identities)
}

#[tauri::command]
fn export_receiving_contact(
    state: State<'_, AppState>,
    identity_id: String,
    output_path: String,
) -> Result<String, String> {
    if output_path.trim().is_empty() {
        return Err("Choose where to save the contact card".into());
    }
    if std::path::Path::new(&output_path)
        .extension()
        .and_then(|value| value.to_str())
        != Some("nqcontact")
    {
        return Err("Contact cards must use the .nqcontact extension".into());
    }
    let identity_id = identity_id
        .parse::<IdentityId>()
        .map_err(|_| "The receiving identity identifier is invalid".to_string())?;
    let guard = active_session(&state)?;
    let session = guard.as_ref().expect("active_session checked");
    let card = contact_card_for_identity(session, &identity_id)?;
    let encoded = serde_json::to_vec_pretty(&card)
        .map_err(|_| "Could not encode the contact card".to_string())?;
    sharing::write_new_atomic(&output_path, &encoded)?;
    touch_activity(&state)?;
    Ok(format!("Contact card exported to {output_path}"))
}

#[tauri::command]
fn import_contact_card(path: String) -> Result<sharing::ContactCard, String> {
    if path.trim().is_empty() {
        return Err("Select a contact card to import".into());
    }
    sharing::read_contact_card(path)
}

#[tauri::command]
async fn create_shared_package(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
    contact_card_json: String,
    operation_id: String,
) -> Result<String, String> {
    if input_path.trim().is_empty() || output_path.trim().is_empty() {
        return Err("Choose the file to share and a new package destination".into());
    }
    if std::path::Path::new(&output_path)
        .extension()
        .and_then(|value| value.to_str())
        != Some("nqx")
    {
        return Err("Shared packages must use the .nqx extension".into());
    }
    with_operation(&app, &state, &operation_id, |control| {
        let _session = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        let card: sharing::ContactCard = serde_json::from_str(&contact_card_json)
            .map_err(|_| "The selected contact card is invalid".to_string())?;
        emit_operation_event(&app, &operation_id, "running", None, None);
        sharing::create_shared_package_file(&input_path, &output_path, &card, || {
            control.is_cancelled()
        })?;
        touch_activity(&state)?;
        Ok("Encrypted package created. The original file was not modified.".into())
    })
}

#[tauri::command]
async fn open_shared_package(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
    key_id: String,
    operation_id: String,
) -> Result<String, String> {
    if input_path.trim().is_empty() || output_path.trim().is_empty() || key_id.trim().is_empty() {
        return Err("Choose the shared package, receiving identity and a new destination".into());
    }
    let key_id = key_id
        .parse::<KeyId>()
        .map_err(|_| "The selected receiving key identifier is invalid".to_string())?;
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        let package = sharing::read_bounded_file(
            &input_path,
            sharing::MAX_SHARED_PACKAGE_BYTES,
            "shared package",
        )?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let belongs_to_active_identity = session.list_identities().any(|identity| {
            identity.is_active()
                && session
                    .find_key(&key_id)
                    .is_some_and(|key| key.metadata.owner.as_deref() == Some(identity.id.as_str()))
        });
        if !belongs_to_active_identity {
            return Err("The selected key is not attached to an active receiving identity".into());
        }
        let plaintext = session
            .open_envelope_with_key_agreement(&key_id, package.as_slice())
            .map_err(|error| format!("Could not open the shared package: {error}"))?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        sharing::write_new_atomic(&output_path, plaintext.as_slice())?;
        touch_activity(&state)?;
        Ok("Shared file recovered successfully. Sender identity has not been verified.".into())
    })
}

#[tauri::command]
async fn encrypt_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    source_path: String,
    vault_folder_path: String,
    operation_id: String,
) -> Result<String, String> {
    if source_path.trim().is_empty() || vault_folder_path.trim().is_empty() {
        return Err("Indica la carpeta original y dónde guardar el vault de carpeta".into());
    }
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let key_id = active_file_key(session)?;
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        let count = nexus_q_vault_spike::folder_manifest::encrypt_folder_controlled(
            session,
            &key_id,
            std::path::Path::new(&source_path),
            std::path::Path::new(&vault_folder_path),
            control,
            &mut reporter,
        )
        .map_err(|error| format!("No se pudo cifrar la carpeta: {error}"))?;
        Ok(format!("Carpeta cifrada: {count} archivos protegidos"))
    })
}

#[tauri::command]
async fn decrypt_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    vault_folder_path: String,
    destination_path: String,
    operation_id: String,
) -> Result<String, String> {
    if vault_folder_path.trim().is_empty() || destination_path.trim().is_empty() {
        return Err("Indica la carpeta vault y el destino de restauración".into());
    }
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        let count = nexus_q_vault_spike::folder_manifest::decrypt_folder_controlled(
            session,
            std::path::Path::new(&vault_folder_path),
            std::path::Path::new(&destination_path),
            control,
            &mut reporter,
        )
        .map_err(|error| format!("No se pudo restaurar la carpeta: {error}"))?;
        Ok(format!("Carpeta restaurada: {count} archivos recuperados"))
    })
}

#[tauri::command]
async fn export_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    backup_path: String,
    backup_passphrase: String,
    backup_passphrase_confirmation: String,
    operation_id: String,
) -> Result<String, String> {
    if backup_path.trim().is_empty() {
        return Err("Indica dónde guardar el backup".into());
    }
    if backup_passphrase.len() < 12 {
        return Err("Usa una contraseña de backup de al menos 12 bytes".into());
    }
    with_operation(&app, &state, &operation_id, |control| {
        let guard = active_session(&state)?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        let session = guard.as_ref().expect("active_session checked");
        let passphrase = confirmed_secret(
            backup_passphrase,
            backup_passphrase_confirmation,
            "La contraseña del backup",
        )?;
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        backup::export_controlled(
            session,
            &backup_path,
            passphrase.as_slice(),
            control,
            &mut reporter,
        )
        .map_err(|error| format!("No se pudo crear el backup: {error}"))?;
        Ok("Backup cifrado creado".into())
    })
}

#[tauri::command]
async fn import_backup(
    app: AppHandle,
    backup_path: String,
    backup_passphrase: String,
    new_vault_path: String,
    new_vault_password: String,
    new_vault_password_confirmation: String,
    operation_id: String,
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
    let state = app.state::<AppState>();
    with_operation(&app, &state, &operation_id, |control| {
        let mut guard = state
            .session
            .lock()
            .map_err(|_| "El estado seguro de la aplicación no está disponible".to_string())?;
        if control.is_cancelled() {
            return Err("operation cancelled".into());
        }
        emit_operation_event(&app, &operation_id, "running", None, None);
        if guard.is_some() {
            return Err("Bloquea el vault actual antes de importar un backup".into());
        }
        let passphrase = secret_bytes(backup_passphrase);
        let password = confirmed_secret(
            new_vault_password,
            new_vault_password_confirmation,
            "La nueva contraseña del vault",
        )?;
        let mut reporter =
            |progress| emit_operation_event(&app, &operation_id, "running", Some(progress), None);
        let vault = backup::import_controlled(
            &backup_path,
            passphrase.as_slice(),
            &new_vault_path,
            password.as_slice(),
            control,
            &mut reporter,
        )
        .map_err(|error| format!("No se pudo restaurar el backup: {error}"))?;
        let session = vault
            .unlock(password.as_slice())
            .map_err(|e| format!("No se pudo verificar el vault restaurado: {e}"))?;
        *guard = Some(session);
        touch_activity(&state)?;
        Ok("Backup restaurado en un nuevo vault y verificado".into())
    })
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
        "encrypted" | "shared" => app
            .dialog()
            .file()
            .set_title("Seleccionar archivo cifrado")
            .add_filter("Archivo NEXUS-Q", &["nqx"])
            .blocking_pick_file(),
        "contact" => app
            .dialog()
            .file()
            .set_title("Seleccionar tarjeta de contacto")
            .add_filter("Tarjeta de contacto NEXUS-Q", &["nqcontact"])
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
        "shared" => app
            .dialog()
            .file()
            .set_title("Guardar paquete cifrado")
            .add_filter("Paquete NEXUS-Q", &["nqx"])
            .set_file_name("shared-file.nqx")
            .blocking_save_file(),
        "contact" => app
            .dialog()
            .file()
            .set_title("Guardar tarjeta de contacto")
            .add_filter("Tarjeta de contacto NEXUS-Q", &["nqcontact"])
            .set_file_name("nexus-q-contact.nqcontact")
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
            cancel_operation,
            encrypt_file,
            decrypt_file,
            create_receiving_identity,
            list_receiving_identities,
            export_receiving_contact,
            import_contact_card,
            create_shared_package,
            open_shared_package,
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
                match resume_session(&state) {
                    Ok(true) => {
                        let _ = app.emit("vault-auto-locked", "system-resume");
                    }
                    Ok(false) => {}
                    Err(error) => {
                        let _ = app.emit("vault-lock-failed", error);
                    }
                }
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
    use super::{
        AppState, AutoLockOutcome, auto_lock_state, idle_timeout_from_seconds, inactivity_elapsed,
        lock_session, lock_session_slot, record_activity_state, resume_session, touch_activity,
    };
    use nexusq_core::vault::{OperationControl, Vault};
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::{Duration, Instant},
    };

    fn state_with_idle_unlocked_vault(dir: &tempfile::TempDir) -> AppState {
        let vault_path = dir.path().join("vault.nqv");
        Vault::create(&vault_path, b"test-password", None).unwrap();
        let session = Vault::open(&vault_path)
            .unwrap()
            .unlock(b"test-password")
            .unwrap();

        let state = AppState::default();
        *state.session.lock().unwrap() = Some(session);
        *state.idle_timeout.lock().unwrap() = Duration::from_secs(60);
        *state.last_activity.lock().unwrap() = Instant::now() - Duration::from_secs(61);
        state
    }

    #[test]
    fn cancellation_registry_remains_available_while_session_is_locked() {
        let state = AppState::default();
        let control = OperationControl::new();
        state
            .operations
            .lock()
            .unwrap()
            .insert("test-operation".into(), control.clone());
        let _session_guard = state.session.lock().unwrap();
        state
            .operations
            .lock()
            .unwrap()
            .get("test-operation")
            .unwrap()
            .cancel();
        assert!(control.is_cancelled());
    }

    #[test]
    fn accepts_only_supported_idle_timeouts() {
        for seconds in [0, 60, 300, 600, 900, 1800] {
            assert_eq!(
                idle_timeout_from_seconds(seconds),
                Ok(Duration::from_secs(seconds))
            );
        }
        for seconds in [30, 120, 3600] {
            assert!(idle_timeout_from_seconds(seconds).is_err());
        }
    }

    #[test]
    fn recent_activity_resets_the_inactivity_deadline() {
        let state = AppState::default();
        let timeout = Duration::from_secs(300);
        *state.last_activity.lock().unwrap() = Instant::now() - timeout;
        assert!(inactivity_elapsed(
            *state.last_activity.lock().unwrap(),
            Instant::now(),
            timeout
        ));

        touch_activity(&state).unwrap();
        assert!(!inactivity_elapsed(
            *state.last_activity.lock().unwrap(),
            Instant::now(),
            timeout
        ));
    }

    #[test]
    fn activity_is_recorded_while_an_operation_holds_the_session_mutex() {
        let state = AppState::default();
        *state.last_activity.lock().unwrap() = Instant::now() - Duration::from_secs(60);
        let operation_guard = state.session.lock().unwrap();

        record_activity_state(&state).unwrap();

        assert!(state.last_activity.lock().unwrap().elapsed() < Duration::from_secs(1));
        drop(operation_guard);
    }

    #[test]
    fn idle_lock_happens_only_once_for_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_idle_unlocked_vault(&dir);

        assert_eq!(auto_lock_state(&state), AutoLockOutcome::Locked);
        assert!(state.session.lock().unwrap().is_none());
        assert_eq!(auto_lock_state(&state), AutoLockOutcome::NotDue);
    }

    #[test]
    fn idle_lock_waits_for_an_operation_holding_the_session_mutex() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(state_with_idle_unlocked_vault(&dir));
        let operation_guard = state.session.lock().unwrap();
        let worker_state = Arc::clone(&state);
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            result_tx.send(auto_lock_state(&worker_state)).unwrap();
        });

        started_rx.recv().unwrap();
        assert!(result_rx.recv_timeout(Duration::from_millis(100)).is_err());
        drop(operation_guard);

        assert_eq!(
            result_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            AutoLockOutcome::Locked
        );
        worker.join().unwrap();
        assert!(state.session.lock().unwrap().is_none());
    }

    #[test]
    fn failed_auto_lock_preserves_session_and_defers_retries() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_idle_unlocked_vault(&dir);
        let vault_path = dir.path().join("vault.nqv");
        std::fs::remove_file(&vault_path).unwrap();
        std::fs::create_dir(&vault_path).unwrap();

        assert!(matches!(
            auto_lock_state(&state),
            AutoLockOutcome::Failed(_)
        ));
        assert!(state.session.lock().unwrap().is_some());
        assert_eq!(auto_lock_state(&state), AutoLockOutcome::NotDue);
    }

    #[test]
    fn resume_locks_an_open_session_and_refreshes_activity() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_idle_unlocked_vault(&dir);

        assert!(resume_session(&state).unwrap());
        assert!(state.session.lock().unwrap().is_none());
        assert!(state.last_activity.lock().unwrap().elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn failed_resume_lock_preserves_session_and_inactivity_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_idle_unlocked_vault(&dir);
        let previous_activity = *state.last_activity.lock().unwrap();
        let vault_path = dir.path().join("vault.nqv");
        std::fs::remove_file(&vault_path).unwrap();
        std::fs::create_dir(&vault_path).unwrap();

        assert!(resume_session(&state).is_err());
        assert!(state.session.lock().unwrap().is_some());
        assert_eq!(*state.last_activity.lock().unwrap(), previous_activity);
        assert!(state.auto_lock_retry_after.lock().unwrap().is_some());
    }

    #[test]
    fn manual_and_exit_lock_path_uses_recoverable_session_locking() {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.nqv");
        Vault::create(&vault_path, b"test-password", None).unwrap();
        let session = Vault::open(&vault_path)
            .unwrap()
            .unlock(b"test-password")
            .unwrap();
        let state = AppState::default();
        *state.session.lock().unwrap() = Some(session);

        assert!(lock_session(&state).unwrap());
        assert!(state.session.lock().unwrap().is_none());
    }

    #[test]
    fn successful_lock_removes_the_session_from_application_state() {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.nqv");
        Vault::create(&vault_path, b"test-password", None).unwrap();
        let session = Vault::open(&vault_path)
            .unwrap()
            .unlock(b"test-password")
            .unwrap();

        let mut slot = Some(session);
        assert!(lock_session_slot(&mut slot).unwrap());
        assert!(slot.is_none());
    }

    #[test]
    fn failed_lock_keeps_the_session_in_application_state() {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.nqv");
        Vault::create(&vault_path, b"test-password", None).unwrap();
        let session = Vault::open(&vault_path)
            .unwrap()
            .unlock(b"test-password")
            .unwrap();

        // Replace the vault file with a directory so the lock write fails.
        std::fs::remove_file(&vault_path).unwrap();
        std::fs::create_dir(&vault_path).unwrap();

        let mut slot = Some(session);
        assert!(lock_session_slot(&mut slot).is_err());
        assert!(
            slot.is_some(),
            "failed locking must not discard the session"
        );
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

#[cfg(test)]
mod password_confirmation_tests {
    use super::confirmed_secret;

    #[test]
    fn accepts_matching_passwords_without_normalizing_whitespace() {
        let password = confirmed_secret(
            "  exact password  ".into(),
            "  exact password  ".into(),
            "password",
        )
        .expect("matching values should be accepted");
        assert_eq!(password.as_slice(), b"  exact password  ");
    }

    #[test]
    fn rejects_mismatched_passwords() {
        assert!(
            confirmed_secret(
                "first password".into(),
                "second password".into(),
                "password"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_empty_passwords_and_whitespace_mismatches() {
        assert!(confirmed_secret(String::new(), String::new(), "password").is_err());
        assert!(confirmed_secret(String::new(), "non-empty".into(), "password").is_err());
        assert!(confirmed_secret("password".into(), " password".into(), "password").is_err());
    }
}

#[cfg(test)]
mod receiving_identity_failure_tests {
    use super::*;
    use nexusq_core::vault::{KeyStatus, VaultError};
    use tempfile::TempDir;

    #[test]
    fn identity_creation_failure_best_effort_destroys_the_generated_key() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("receiving.nqv");
        let vault = Vault::create(&path, b"test-password", None).unwrap();
        let mut session = vault.unlock(b"test-password").unwrap();

        let error = create_receiving_identity_records_with(
            &mut session,
            "Alice".into(),
            |_session, _label| Err(VaultError::WrongPassword),
            |session, key_id| session.destroy_key(key_id, DestructionConfirmation::Explicit),
        )
        .unwrap_err();

        assert!(error.contains("was destroyed"));
        let keys: Vec<_> = session.list_keys().collect();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].status(), KeyStatus::Destroyed);
        assert!(session.list_identities().next().is_none());
    }

    #[test]
    fn identity_creation_failure_reports_when_best_effort_key_cleanup_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("receiving.nqv");
        let vault = Vault::create(&path, b"test-password", None).unwrap();
        let mut session = vault.unlock(b"test-password").unwrap();

        let error = create_receiving_identity_records_with(
            &mut session,
            "Alice".into(),
            |_session, _label| Err(VaultError::WrongPassword),
            |_session, _key_id| Err(VaultError::WrongPassword),
        )
        .unwrap_err();

        assert!(error.contains("Best-effort cleanup"));
        assert!(error.contains("Cleanup could not be confirmed"));
        assert!(error.contains("after reopening the vault"));
        let keys: Vec<_> = session.list_keys().collect();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].status(), KeyStatus::Generated);
        assert!(session.list_identities().next().is_none());
    }
}
