# NEXUS-Q Vault v0.2.0 release checklist

**Status:** preparation in progress. No `v0.2.0` tag or public release is created by this checklist.

## Source and scope
- [x] Base release range is `v0.1.0..0a6dbad55f8ed86e97a18dbf19728404d74817c6` (34 commits).
- [x] PRs #22–#33 are merged; no open pull requests were found during the 2026-11-12 audit.
- [x] Remaining open issues #6–#15 are future language requests (Italian, Japanese, Korean, Simplified/Traditional Chinese, Arabic, Hindi, Indonesian, Turkish and Russian); none is a dependency of the current release scope.
- [x] Core repository is not modified. CI and release builds use the immutable `nexus-q` revision `458fc393992f4b8030ff594a408620058032e214`.

## Release content
- [x] `CHANGELOG.md` records the v0.2.0 changes.
- [x] README describes current features and all seven UI languages.
- [x] Version metadata is aligned to `0.2.0`; Windows WiX ProductVersion is `0.2.0.1`.
- [x] Release workflow has a manual candidate-build path that does not publish; tag-triggered publishing creates a draft release.

## Required validation before tagging
- [ ] Run root Rust checks with the pinned sibling Core checkout available: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`.
- [ ] Run desktop Rust checks: `cargo fmt --manifest-path apps/desktop/Cargo.toml -- --check`, `cargo clippy --manifest-path apps/desktop/Cargo.toml --locked --all-targets -- -D warnings`, `cargo test --manifest-path apps/desktop/Cargo.toml --locked --bin nexus-q-vault-desktop`.
- [ ] Run frontend tests: `node --test apps/desktop/ui/i18n/i18n.test.js apps/desktop/ui/password-confirmation.test.js`.
- [ ] Review the recipient-sharing warning: recipient encryption protects the receiving identity but does not authenticate the sender; fingerprint verification must use a separate trusted channel.
- [ ] Run release workflow manually and inspect Windows MSI/NSIS and macOS DMG candidate artifacts.
- [ ] Build Linux `.deb` and `.AppImage` on the project's Zorin OS 18.1 build host from the exact same Vault commit and pinned Core revision.
- [ ] Smoke-test installers and application startup on Windows, macOS and Linux. Compilation alone is not an installer/runtime smoke test.

## Artifact finalization
- [ ] Collect exactly the intended Windows `.msi` and `.exe`, macOS `.dmg`, Linux `.deb` and `.AppImage` installers.
- [ ] Include `BUILD-INFO-windows.txt`, `BUILD-INFO-macos.txt` and `BUILD-INFO-linux.txt` with identical `version=0.2.0` and the same 40-character Vault commit hash.
- [ ] Run `scripts/prepare-release-assets.sh <release-assets-directory>` from the exact release-commit checkout; it rejects missing formats, missing metadata, commit/version mismatches and metadata that does not match the current checkout, then writes `SHA256SUMS.txt`.
- [ ] Verify the checksum file on a clean copy of the complete artifact directory. Checksums detect corruption; they are not signatures or provenance attestations.
- [ ] Review artifact names, formats, sizes and checksums, then upload the Linux installers and final checksum file to the draft release.

## Final authorization gate
- [ ] Review the final diff, exact release commit, CI run, release notes, platform smoke tests and all five installers.
- [ ] Obtain explicit authorization before creating/pushing tag `v0.2.0`.
- [ ] Obtain explicit authorization before publishing the draft GitHub Release.

## Linux build metadata example
From the exact release commit on the Zorin build host, after building the Linux installers:

```sh
mkdir -p release-assets
printf 'version=0.2.0\ncommit=%s\nplatform=linux\n' "$(git rev-parse HEAD)" > release-assets/BUILD-INFO-linux.txt
```
