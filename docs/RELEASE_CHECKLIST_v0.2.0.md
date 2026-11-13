# NEXUS-Q Vault v0.2.0 release checklist

**Status:** completed and publicly released on 2026-11-13.

## Release identity
- [x] Version: `0.2.0`.
- [x] Release tag: `v0.2.0`.
- [x] Release commit: `3d174a3338bf775eb46d5a836607689994887b78`.
- [x] GitHub Release is published (not a draft and not a prerelease).
- [x] Public release: https://github.com/lecodev-26/nexus-q-vault/releases/tag/v0.2.0
- [x] Core repository was not modified.

## Release content
- [x] Changelog updated for v0.2.0 and committed to `main` in `dea0a08`.
- [x] README and release notes describe the shipped feature set.
- [x] Version metadata is aligned to `0.2.0`; Windows WiX ProductVersion is `0.2.0.1`.
- [x] Configurable automatic vault inactivity locking and system-event lock handling.
- [x] Recovery-safe lock behavior and sensitive-state cleanup on failed transitions.
- [x] Cancellable operations center for supported long-running vault operations.
- [x] Recipient-encrypted file sharing packages, without a hosted service.
- [x] Seven interface languages: English, Spanish, Catalan, French, German, Portuguese (Portugal) and Portuguese (Brazil).
- [x] Independent-process backup recovery coverage and clearer recovery guidance.
- [x] Hardened shared-file publication, temporary-file permissions, path handling and failure cleanup.

## CI and platform builds
- [x] GitHub Actions release run completed successfully: https://github.com/lecodev-26/nexus-q-vault/actions/runs/43338753807
- [x] Release run ID: `43338753807`.
- [x] Release workflow completed with `success` on commit `3d174a3338bf775eb46d5a836607689994887b78`.
- [x] Linux build job completed successfully.
- [x] Windows build job completed successfully.
- [x] macOS build job completed successfully.
- [x] Release validation/publishing job completed successfully.
- [x] All five workflow jobs completed successfully.

## Published release assets
The public release contains the following nine assets:

- [x] `NEXUS-Q.Vault_0.2.0_amd64.deb`
- [x] `NEXUS-Q.Vault_0.2.0_amd64.AppImage`
- [x] `NEXUS-Q.Vault_0.2.0_aarch64.dmg` (macOS Apple Silicon)
- [x] `NEXUS-Q.Vault_0.2.0_x64-setup.exe`
- [x] `NEXUS-Q.Vault_0.2.0_x64_en-US.msi`
- [x] `BUILD-INFO-linux.txt`
- [x] `BUILD-INFO-macos.txt`
- [x] `BUILD-INFO-windows.txt`
- [x] `SHA256SUMS.txt`

## Artifact verification
- [x] Release asset filenames were normalized by `scripts/prepare-release-assets.sh` before checksum generation.
- [x] All three BUILD-INFO files report version `0.2.0`.
- [x] All three BUILD-INFO files identify release commit `3d174a3338bf775eb46d5a836607689994887b78` and their respective platforms.
- [x] All nine release assets were downloaded for verification.
- [x] All eight checksum entries in `SHA256SUMS.txt` passed local `sha256sum -c` verification (five installers and three BUILD-INFO files).
- [x] Checksum verification confirms file integrity against the published checksum list; it is not a digital signature or an independent provenance attestation.

## Post-release notes
- [x] The release is public and available to users at the release URL above.
- [x] Existing v0.1.0 and v0.2.0 installations do not have an integrated updater; users must download and install newer versions manually.
- [x] Follow-up issue opened for a Tauri integrated updater in a future release: https://github.com/lecodev-26/nexus-q-vault/issues/37.
- [x] The initial transition to a version containing the updater will require a manual installation; integrated updates can be used for subsequent releases.

## Release outcome

**v0.2.0 is complete and published.** No further v0.2.0 tagging or publication authorization is pending.
