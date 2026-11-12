# Changelog

All notable changes to NEXUS-Q Vault are documented here.

## [0.2.0] — 2026-11-12 (release candidate; not published)

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

### Tests and CI
- Added frontend tests for password-confirmation flows and locale completeness/runtime behavior.
- Added recovery, malformed-share-package, disk-full publication and Windows hardening tests.
- Current `main` CI run for commit `0a6dbad55f8ed86e97a18dbf19728404d74817c6` completed successfully on 2026-11-12.
- Release candidate validation must still run on the exact release-preparation commit; Linux installer generation and cross-platform artifact verification remain release gates.

### Security and compatibility notes
- This remains early-access software and has not received an independent security audit.
- Encrypted file payloads and folder-vault objects remain separate from `.nqb` vault/key recovery backups and must be backed up independently.
- Existing folder-format limitations remain; see `README.md`, `THREAT_MODEL.md` and `docs/FOLDER_FORMAT_V1.md`.
- No tag or public release has been created for v0.2.0.

## [0.1.0] — 2026-11-06

First public early-access release. See the [v0.1.0 GitHub release](https://github.com/lecodev-26/nexus-q-vault/releases/tag/v0.1.0).
