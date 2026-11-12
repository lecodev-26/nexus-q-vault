# Product specification — MVP 0.2

## Target user
A person with no command-line or cryptography experience who wants to protect documents, photos and personal backups on a computer they control.

## Single MVP use case
A **local encrypted file vault with recipient-encrypted file exchange**. No network account or cloud service is required.

## Critical user journeys
1. **Create a vault:** choose a location, create a strong password, create and verify a recovery method, then receive clear confirmation of what is saved.
2. **Protect and restore files:** add files or folders, show progress, preserve originals unless the user explicitly chooses otherwise, and restore files to a chosen destination.
3. **Recover access:** use the documented recovery mechanism or a separately encrypted backup, with clear warnings about what can and cannot be recovered.

## MVP requirements
- Create, open, lock and unlock a versioned local vault, including configurable inactivity and system-event locking.
- Cancel supported long-running operations from the operations center.
- Create recipient-encrypted file packages and decrypt packages addressed to the local receiving identity.
- Encrypt and decrypt individual files through nexusq-core.
- Protect folder contents through a versioned encrypted manifest (implemented in the prototype; format migration and security review pending).
- Create, verify and restore an encrypted backup.
- Detect corruption and fail closed: never return partial plaintext.
- Make export/migration possible without a proprietary service.
- Telemetry disabled by default; never collect content, filenames, paths, keys or recovery data.

## Recovery design constraints
- Recovery must be designed and tested before broad release.
- Never equate a backup passphrase with the vault password.
- Recovery phrases must encode sufficient entropy and have a versioned, checksummed, human-verifiable representation.
- The phrase must not be logged, transmitted or stored by the app in plaintext.
- The precise recovery mechanism remains an open design decision; do not promise password reset or phrase-based recovery until core support and tests exist.

## Logical data model
- Vault: format version, encrypted payload, KDF parameters and minimal public header fields.
- Protected file: opaque identifier, encrypted payload and integrity data.
- Manifest: encrypted mapping of opaque identifiers to file metadata and content references.
- Activity: local-only privacy-minimized events; never include file contents, names, absolute paths, secrets or recovery material.

## Current prototype limitations

The desktop application encrypts individual files to sidecar `.nqx` envelopes, supports recipient-encrypted file exchange packages, and can encrypt folders into `manifest.nqm` plus `objects/<opaque-id>.nqx`. It includes a cancellable operations center, configurable inactivity locking and seven UI languages (English, Spanish, Catalan, French, German, Portuguese (Portugal) and Portuguese (Brazil)). Original names and relative paths are inside the authenticated manifest envelope; object names are opaque. It rejects symlinks, omits empty directories, accepts only portable UTF-8 paths and limits each file to 32 MiB. Native file and folder selectors are wired through the Tauri dialog plugin; onboarding and accessibility testing remain pending. Large-file streaming is not implemented; see the [NQX2 proposal](docs/STREAMING_FORMAT_PROPOSAL.md). Its `.nqb` backup contains vault/key state, not the `.nqx` payloads or folder object directories; users must preserve those separately. The manifest has an initial version number, but migration/upgrade behavior is not yet implemented; see [folder format v1](docs/FOLDER_FORMAT_V1.md). The backup-based recovery journey is a proposal in [docs/RECOVERY_DESIGN.md](docs/RECOVERY_DESIGN.md), not yet user-tested or approved for beta.

## Explicitly out of scope
Synchronization, cloud storage, public sharing links, collaboration, teams, enterprise policy, remote administration and multi-device identity. Recipient-encrypted packages are exchanged out of band and do not provide sender authentication.

## Acceptance criteria
- A new user can create and unlock a vault without a terminal.
- Wrong passwords and modified ciphertext fail safely.
- Files round-trip byte-for-byte.
- A backup restores into a new vault with separately supplied credentials.
- Recovery instructions are understandable and validated by usability testing.
- Format and migration behavior are documented and tested.
