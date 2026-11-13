# Changelog

All notable changes to NEXUS-Q Vault are documented here.

## [0.2.0] — 2026-11-13

### Added
- Configurable automatic vault inactivity locking and system-event lock handling.
- Recovery-safe lock behavior that preserves an unlocked session when a lock attempt fails and clears sensitive state on failed transitions.
- Cancellable operations center for supported long-running vault operations.
- Recipient-encrypted file sharing packages for exchanging protected files without a hosted service.
- Modular localization with Catalan, French, German, Portuguese (Portugal) and Portuguese (Brazil), alongside English and Spanish.
- Independent-process backup recovery coverage and clearer recovery guidance.

### Improved
- Hardened shared-file publication, temporary-file permissions, path handling and failure cleanup.
- Improved password-confirmation validation before vault creation, backup export and backup restoration.
- Refined German and Portuguese interface wording and regional variants.
- Expanded documentation for automatic locking, cancellable operations, recovery and encrypted file exchange.
- Normalized release asset filenames before generating SHA-256 checksums.

### Tests and CI
- Added frontend tests for password-confirmation flows and locale completeness/runtime behavior.
- Added recovery, malformed-share-package, disk-full publication and Windows hardening tests.
- Release workflow for commit `3d174a3338bf775eb46d5a836607689994887b78` completed successfully on 2026-11-13.
- Linux, Windows and macOS build jobs and release validation completed successfully.
- Published release artifacts and their SHA-256 checksums were generated; all eight checksum entries were verified successfully.

### Security and compatibility notes
- This remains early-access software and has not received an independent security audit.
- Encrypted file payloads and folder-vault objects remain separate from `.nqb` vault/key recovery backups and must be backed up independently.
- Recipient-encrypted file exchange does not authenticate the sender; verify recipient fingerprints through a separate trusted channel and keep a separate `.nqb` backup of receiving keys.
- Existing folder-format limitations remain; see `README.md`, `THREAT_MODEL.md` and `docs/FOLDER_FORMAT_V1.md`.

Published release: [NEXUS-Q Vault v0.2.0](https://github.com/lecodev-26/nexus-q-vault/releases/tag/v0.2.0).

## [0.1.0] — 2026-11-06

First public early-access release. See the [v0.1.0 GitHub release](https://github.com/lecodev-26/nexus-q-vault/releases/tag/v0.1.0).
