use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use nexusq_core::{crypto::hash::sha256, identity::IdentityId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const MAX_SHARED_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SHARED_PACKAGE_BYTES: u64 = MAX_SHARED_FILE_BYTES + 16 * 1024;
pub const MAX_CONTACT_CARD_BYTES: u64 = 64 * 1024;
const CONTACT_CARD_FORMAT: &str = "nexusq-contact-card";
const CONTACT_CARD_VERSION: u8 = 1;
const CONTACT_CARD_ALGORITHM: &str = "mlkem768-xwing-v1";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactCard {
    pub format: String,
    pub version: u8,
    pub identity_id: String,
    pub label: String,
    pub key_id: String,
    pub algorithm: String,
    pub public_key: Vec<u8>,
    pub fingerprint: String,
}

impl ContactCard {
    pub fn new(
        identity_id: String,
        label: String,
        key_id: String,
        public_key: Vec<u8>,
    ) -> Result<Self, String> {
        let mut card = Self {
            format: CONTACT_CARD_FORMAT.to_string(),
            version: CONTACT_CARD_VERSION,
            identity_id,
            label,
            key_id,
            algorithm: CONTACT_CARD_ALGORITHM.to_string(),
            public_key,
            fingerprint: String::new(),
        };
        card.fingerprint = card.computed_fingerprint();
        card.validate()?;
        Ok(card)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != CONTACT_CARD_FORMAT || self.version != CONTACT_CARD_VERSION {
            return Err("Unsupported contact card format or version".into());
        }
        if self.algorithm != CONTACT_CARD_ALGORITHM {
            return Err("The contact card uses an unsupported key algorithm".into());
        }
        self.identity_id
            .parse::<IdentityId>()
            .map_err(|_| "The contact card has an invalid identity identifier".to_string())?;
        if self.label.trim().is_empty()
            || self.label.len() > 80
            || self.label.chars().any(char::is_control)
        {
            return Err("The contact card has an invalid display name".into());
        }
        let parsed_key_id = self
            .key_id
            .parse::<nexusq_core::vault::KeyId>()
            .map_err(|_| "The contact card has an invalid key identifier".to_string())?;
        if parsed_key_id.algorithm() != "mlkem768" {
            return Err("The contact card key is not ML-KEM-768".into());
        }
        if self.public_key.len() != nexusq_core::crypto::kem::hybrid::PUBLIC_KEY_LEN {
            return Err("The contact card public key has an invalid length".into());
        }
        if self.fingerprint != self.computed_fingerprint() {
            return Err("The contact card fingerprint does not match its contents".into());
        }
        Ok(())
    }

    fn computed_fingerprint(&self) -> String {
        let mut input = Vec::with_capacity(
            32 + self.identity_id.len()
                + self.label.len()
                + self.key_id.len()
                + self.public_key.len(),
        );
        input.extend_from_slice(b"NEXUS-Q CONTACT CARD V1\0");
        input.extend_from_slice(&(self.identity_id.len() as u32).to_be_bytes());
        input.extend_from_slice(self.identity_id.as_bytes());
        input.extend_from_slice(&(self.label.len() as u32).to_be_bytes());
        input.extend_from_slice(self.label.as_bytes());
        input.extend_from_slice(&(self.key_id.len() as u32).to_be_bytes());
        input.extend_from_slice(self.key_id.as_bytes());
        input.extend_from_slice(&(self.public_key.len() as u32).to_be_bytes());
        input.extend_from_slice(&self.public_key);
        format!("sha256:{}", encode_hex(&sha256(&input)))
    }
}

pub fn read_contact_card(path: impl AsRef<Path>) -> Result<ContactCard, String> {
    let bytes = read_bounded_file(path, MAX_CONTACT_CARD_BYTES, "contact card")?;
    let card: ContactCard = serde_json::from_slice(&bytes)
        .map_err(|_| "The selected file is not a valid NEXUS-Q contact card".to_string())?;
    card.validate()?;
    Ok(card)
}

pub fn read_bounded_file(
    path: impl AsRef<Path>,
    max_bytes: u64,
    description: &str,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let path = path.as_ref();
    // Open without following links/reparse points, then validate the opened
    // handle. Checking the path before File::open would leave a TOCTOU window
    // in which an attacker could replace a regular file with a symlink.
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: open the reparse point itself so the
        // handle metadata check below can reject symlinks and other reparse points.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not safely open the {description}: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("Could not inspect the opened {description}: {error}"))?;
    #[cfg(windows)]
    let is_link_or_reparse_point = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    #[cfg(not(windows))]
    let is_link_or_reparse_point = metadata.file_type().is_symlink();
    if is_link_or_reparse_point || !metadata.is_file() {
        return Err(format!(
            "The {description} must be a regular file, not a symbolic link or reparse point"
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "The {description} exceeds the supported size limit"
        ));
    }
    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(bytes.as_mut())
        .map_err(|error| format!("Could not read the {description}: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "The {description} exceeds the supported size limit"
        ));
    }
    Ok(bytes)
}

pub fn create_shared_package_file(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    card: &ContactCard,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), String> {
    card.validate()?;
    if cancelled() {
        return Err("operation cancelled".into());
    }
    let plaintext = read_bounded_file(input_path, MAX_SHARED_FILE_BYTES, "source file")?;
    if cancelled() {
        return Err("operation cancelled".into());
    }
    let package = nexusq_core::vault::envelope::build_envelope_to_public_key(
        &card.public_key,
        plaintext.as_slice(),
        Vec::new(),
    )
    .map_err(|error| format!("Could not encrypt the shared package: {error}"))?;
    if package.len() as u64 > MAX_SHARED_PACKAGE_BYTES {
        return Err("The encrypted package exceeds the supported size limit".into());
    }
    if cancelled() {
        return Err("operation cancelled".into());
    }
    write_new_atomic(output_path, &package)
}

pub fn write_new_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    write_new_atomic_with(path.as_ref(), bytes, |file, payload| {
        file.write_all(payload)
    })
}

#[cfg(windows)]
fn create_private_temp_file(path: &Path) -> std::io::Result<File> {
    use std::{
        ffi::c_void,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
        ptr,
    };

    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        security_descriptor: *mut c_void,
        inherit_handle: i32,
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor: *const u16,
            revision: u32,
            output: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            filename: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *mut SecurityAttributes,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: isize,
        ) -> isize;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    // Protected DACL: grant full access only to the file owner. The descriptor
    // is applied atomically by CreateFileW, before any plaintext is written.
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = ptr::null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut attributes = SecurityAttributes {
        length: std::mem::size_of::<SecurityAttributes>() as u32,
        security_descriptor: descriptor,
        inherit_handle: 0,
    };
    let filename: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            filename.as_ptr(),
            0x4000_0000, // GENERIC_WRITE
            0,           // no sharing while the private file is being populated
            &mut attributes,
            1,    // CREATE_NEW
            0x80, // FILE_ATTRIBUTE_NORMAL
            0,
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    if handle == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle as _) })
}

#[cfg(not(windows))]
fn create_private_temp_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn write_new_atomic_with(
    path: &Path,
    bytes: &[u8],
    mut write_payload: impl FnMut(&mut File, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path.file_name().unwrap_or_default();

    for _ in 0..16 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut temp_name = file_name.to_os_string();
        temp_name.push(format!(".tmp-{}-{timestamp}-{counter}", std::process::id()));
        let temp_path = parent.join(temp_name);
        let mut file = match create_private_temp_file(&temp_path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Could not create temporary output: {error}")),
        };

        let mut published = false;
        let result = (|| -> std::io::Result<()> {
            write_payload(&mut file, bytes)?;
            file.sync_all()?;
            drop(file);
            fs::hard_link(&temp_path, path)?;
            published = true;
            fs::remove_file(&temp_path)?;
            #[cfg(unix)]
            {
                let directory = File::open(parent)?;
                match directory.sync_all() {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::Unsupported => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let mut cleanup_errors = Vec::new();
            if published
                && let Err(error) = fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                cleanup_errors.push(error.to_string());
            }
            if let Err(error) = fs::remove_file(&temp_path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                cleanup_errors.push(error.to_string());
            }
            if !cleanup_errors.is_empty() {
                return Err(format!(
                    "Output publication failed and cleanup also failed: {}",
                    cleanup_errors.join("; ")
                ));
            }
        } else {
            let _ = fs::remove_file(&temp_path);
        }
        return result.map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "The selected destination already exists; no file was overwritten".to_string()
            } else {
                format!("Could not publish the output file: {error}")
            }
        });
    }
    Err("Could not allocate a unique temporary output file".into())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexusq_core::crypto::kem::hybrid;
    use tempfile::TempDir;

    fn sample_card() -> ContactCard {
        let key_pair = hybrid::generate();
        ContactCard::new(
            "nqi_0123456789abcdef0123456789abcdef".into(),
            "Alice".into(),
            "nqk_mlkem768_0123456789abcdef0123456789abcdef".into(),
            key_pair.public_key_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn contact_card_roundtrips_and_detects_tampering() {
        let card = sample_card();
        let encoded = serde_json::to_vec(&card).unwrap();
        let decoded: ContactCard = serde_json::from_slice(&encoded).unwrap();
        decoded.validate().unwrap();

        let mut tampered = decoded;
        tampered.public_key[0] ^= 1;
        assert!(tampered.validate().is_err());
    }

    #[test]
    fn contact_card_file_is_bounded_and_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("recipient.nqcontact");
        let card = sample_card();
        let encoded = serde_json::to_vec_pretty(&card).unwrap();
        write_new_atomic(&path, &encoded).unwrap();
        let decoded = read_contact_card(&path).unwrap();
        assert_eq!(decoded, card);
    }

    #[test]
    fn contact_card_rejects_unknown_versions_and_wrong_key_ids() {
        let mut card = sample_card();
        card.version = 2;
        assert!(card.validate().is_err());

        let mut card = sample_card();
        card.key_id = "nqk_ed25519_0123456789abcdef0123456789abcdef".into();
        card.fingerprint = card.computed_fingerprint();
        assert!(card.validate().is_err());
    }

    #[test]
    fn bounded_reader_rejects_oversized_files_and_symlinks() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("card.json");
        fs::write(&file, b"12345").unwrap();
        assert!(read_bounded_file(&file, 4, "test file").is_err());
        #[cfg(unix)]
        {
            let link = dir.path().join("link.json");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read_bounded_file(&link, 10, "test file").is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_temporary_files_are_created_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("private-plaintext.tmp");
        let file = create_private_temp_file(&path).unwrap();
        let mode = file.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(windows)]
    #[test]
    fn private_temporary_files_have_a_protected_owner_only_dacl() {
        use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr, slice};
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn GetNamedSecurityInfoW(
                object_name: *mut u16,
                object_type: u32,
                security_info: u32,
                owner: *mut *mut c_void,
                group: *mut *mut c_void,
                dacl: *mut *mut c_void,
                sacl: *mut *mut c_void,
                security_descriptor: *mut *mut c_void,
            ) -> u32;
            fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
                security_descriptor: *mut c_void,
                revision: u32,
                security_information: u32,
                output: *mut *mut u16,
                output_len: *mut u32,
            ) -> i32;
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn LocalFree(memory: *mut c_void) -> *mut c_void;
        }
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("private-plaintext.tmp");
        let _file = create_private_temp_file(&path).unwrap();
        let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide_path.push(0);
        let mut descriptor = ptr::null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide_path.as_mut_ptr(),
                1,
                0x0000_0004,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0, "GetNamedSecurityInfoW failed: {status}");
        let mut sddl = ptr::null_mut();
        let mut sddl_len = 0;
        let converted = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                1,
                0x0000_0004,
                &mut sddl,
                &mut sddl_len,
            )
        };
        assert_ne!(converted, 0, "could not convert the file DACL to SDDL");
        let actual =
            String::from_utf16_lossy(unsafe { slice::from_raw_parts(sddl, sddl_len as usize) });
        unsafe {
            LocalFree(sddl.cast());
            LocalFree(descriptor);
        }
        assert!(
            actual.contains("D:P(A;;FA;;;OW)"),
            "unexpected temporary-file DACL: {actual}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_named_pipes_without_blocking() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let dir = TempDir::new().unwrap();
        let fifo = dir.path().join("input.fifo");
        let name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        let result = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
        assert_eq!(result, 0);
        assert!(read_bounded_file(&fifo, 16, "test input").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn bounded_reader_rejects_file_and_directory_reparse_points() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let dir = TempDir::new().unwrap();
        let target_file = dir.path().join("target.txt");
        let target_dir = dir.path().join("target-dir");
        fs::write(&target_file, b"private").unwrap();
        fs::create_dir(&target_dir).unwrap();
        let file_link = dir.path().join("file-link.txt");
        let dir_link = dir.path().join("dir-link");
        symlink_file(&target_file, &file_link)
            .expect("Windows CI must permit creating file symlinks");
        symlink_dir(&target_dir, &dir_link)
            .expect("Windows CI must permit creating directory symlinks");
        assert!(read_bounded_file(&file_link, 64, "test file").is_err());
        assert!(read_bounded_file(&dir_link, 64, "test file").is_err());
    }

    #[test]
    fn independent_vaults_exchange_a_file_and_backup_restores_receiving_keys() {
        use nexusq_core::{
            storage::backup,
            vault::{Algorithm, Purpose, Vault},
        };

        let dir = TempDir::new().unwrap();
        let recipient_path = dir.path().join("recipient.nqv");
        let recipient_vault = Vault::create(&recipient_path, b"recipient-password", None).unwrap();
        let mut recipient = recipient_vault.unlock(b"recipient-password").unwrap();
        let identity_id = recipient.create_identity(Some("Alice".into())).unwrap();
        let key_id = recipient
            .generate_key(Algorithm::MlKem768, Purpose::KeyAgreement)
            .unwrap();
        recipient.activate_key(&key_id).unwrap();
        recipient
            .find_identity_mut(&identity_id)
            .unwrap()
            .unwrap()
            .key_agreement_key = Some(key_id.clone());
        let public_key = recipient
            .find_key(&key_id)
            .unwrap()
            .public_key_bytes()
            .unwrap()
            .to_vec();
        let card = ContactCard::new(
            identity_id.to_string(),
            "Alice".into(),
            key_id.to_string(),
            public_key,
        )
        .unwrap();

        let sender_path = dir.path().join("sender.nqv");
        let sender_vault = Vault::create(&sender_path, b"sender-password", None).unwrap();
        let sender = sender_vault.unlock(b"sender-password").unwrap();
        assert_eq!(sender.key_count(), 0);
        let original = b"byte-for-byte shared payload\0\xff";
        let source_path = dir.path().join("source-ñ.txt");
        let package_path = dir.path().join("shared-file.nqx");
        fs::write(&source_path, original).unwrap();
        create_shared_package_file(&source_path, &package_path, &card, || false).unwrap();
        assert_eq!(fs::read(&source_path).unwrap(), original);
        let package =
            read_bounded_file(&package_path, MAX_SHARED_PACKAGE_BYTES, "shared package").unwrap();
        let recovered = recipient
            .open_envelope_with_key_agreement(&key_id, package.as_slice())
            .unwrap();
        assert_eq!(recovered.as_slice(), original);

        let wrong_key_id = recipient
            .generate_key(Algorithm::MlKem768, Purpose::KeyAgreement)
            .unwrap();
        recipient.activate_key(&wrong_key_id).unwrap();
        assert!(
            recipient
                .open_envelope_with_key_agreement(&wrong_key_id, package.as_slice())
                .is_err()
        );
        let mut tampered_package = package.to_vec();
        let last_byte = tampered_package.len() - 1;
        tampered_package[last_byte] ^= 0x80;
        assert!(
            recipient
                .open_envelope_with_key_agreement(&key_id, &tampered_package)
                .is_err()
        );

        // Malformed, truncated and unsupported-format inputs must fail closed.
        assert!(
            recipient
                .open_envelope_with_key_agreement(&key_id, b"not an NQX package")
                .is_err()
        );
        assert!(
            recipient
                .open_envelope_with_key_agreement(&key_id, &package[..package.len() / 2])
                .is_err()
        );
        let mut unsupported_envelope: nexusq_core::vault::Envelope =
            nexusq_core::vault::from_slice(package.as_slice()).unwrap();
        unsupported_envelope.header.version =
            nexusq_core::vault::ENVELOPE_FORMAT_VERSION.saturating_add(1);
        let unsupported_format = nexusq_core::vault::to_vec(&unsupported_envelope).unwrap();
        assert!(
            recipient
                .open_envelope_with_key_agreement(&key_id, &unsupported_format)
                .is_err()
        );

        // A destination that is a directory must fail without replacing it.
        let blocked_destination = dir.path().join("destination-is-a-directory");
        fs::create_dir(&blocked_destination).unwrap();
        assert!(write_new_atomic(&blocked_destination, original).is_err());
        assert!(blocked_destination.is_dir());

        let empty_source = dir.path().join("empty-file.txt");
        let empty_package = dir.path().join("empty-file.nqx");
        fs::write(&empty_source, []).unwrap();
        create_shared_package_file(&empty_source, &empty_package, &card, || false).unwrap();
        let empty_envelope =
            read_bounded_file(&empty_package, MAX_SHARED_PACKAGE_BYTES, "shared package").unwrap();
        let recovered_empty = recipient
            .open_envelope_with_key_agreement(&key_id, empty_envelope.as_slice())
            .unwrap();
        assert!(recovered_empty.is_empty());

        let backup_path = dir.path().join("recipient.nqb");
        backup::export(&recipient, &backup_path, b"separate-backup-passphrase").unwrap();
        let restored_path = dir.path().join("recipient-restored.nqv");
        let restored_vault = backup::import(
            &backup_path,
            b"separate-backup-passphrase",
            &restored_path,
            b"new-recipient-password",
        )
        .unwrap();
        let restored = restored_vault.unlock(b"new-recipient-password").unwrap();
        let restored_identity = identity_id;
        assert_eq!(
            restored
                .find_identity(&restored_identity)
                .unwrap()
                .key_agreement_key
                .as_ref(),
            Some(&key_id)
        );
        let recovered_after_restore = restored
            .open_envelope_with_key_agreement(&key_id, &package)
            .unwrap();
        assert_eq!(recovered_after_restore.as_slice(), original);
    }

    #[test]
    fn shared_file_limit_rejects_files_larger_than_32_mib() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("oversized.bin");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_SHARED_FILE_BYTES + 1).unwrap();
        assert!(read_bounded_file(&path, MAX_SHARED_FILE_BYTES, "source file").is_err());
    }

    #[test]
    fn empty_and_unicode_named_inputs_are_supported() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("résumé_日本語.txt");
        fs::write(&source, []).unwrap();
        let bytes = read_bounded_file(&source, MAX_SHARED_FILE_BYTES, "source file").unwrap();
        assert!(bytes.is_empty());
        let destination = dir.path().join("recuperado_ñ.txt");
        write_new_atomic(&destination, bytes.as_slice()).unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"");
    }

    #[test]
    fn cancelling_package_creation_never_changes_the_original_or_publishes_output() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source file.txt");
        let output = dir.path().join("share.nqx");
        let original = b"original source bytes";
        fs::write(&source, original).unwrap();
        let card = sample_card();
        let mut checkpoints = 0;
        let result = create_shared_package_file(&source, &output, &card, || {
            checkpoints += 1;
            checkpoints == 3
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&source).unwrap(), original);
        assert!(!output.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn simulated_disk_full_cleans_temporary_output_and_never_publishes_destination() {
        let dir = TempDir::new().unwrap();
        let destination = dir.path().join("disk-full-output.nqx");

        let result = write_new_atomic_with(&destination, b"payload", |file, _payload| {
            file.write_all(b"partial temporary bytes")?;
            Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
        });
        assert!(
            result.is_err(),
            "simulated storage exhaustion must fail publication"
        );
        assert!(!destination.exists());
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporary output was not cleaned up: {leftovers:?}"
        );
    }

    #[test]
    fn atomic_writer_never_overwrites_an_existing_file() {
        let dir = TempDir::new().unwrap();
        let destination = dir.path().join("package.nqx");
        fs::write(&destination, b"original").unwrap();
        assert!(write_new_atomic(&destination, b"replacement").is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
