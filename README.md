# NEXUS-Q Vault

<p align="center">
  <img src="docs/nexus-q-vault-logo.png" alt="NEXUS-Q Vault logo" width="180">
</p>

<p align="center">
  <strong>Protect your files. Without leaving your device.</strong><br>
  A local-first desktop vault for encrypting files and folders, with separately encrypted recovery backups.
</p>

<p align="center">
  <a href="https://nexus-q.link">Website</a> ·
  <a href="https://github.com/lecodev-26/nexus-q-vault/releases">Downloads</a> ·
  <a href="https://github.com/lecodev-26/nexus-q">Cryptographic engine</a> ·
  <a href="SECURITY.md">Security policy</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-orange?logo=rust">
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri">
  <img alt="License MIT" src="https://img.shields.io/badge/license-MIT-green.svg">
  <a href="https://github.com/lecodev-26/nexus-q-vault/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/lecodev-26/nexus-q-vault/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/lecodev-26/nexus-q-vault/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/lecodev-26/nexus-q-vault"></a>
</p>

> **Public release — v0.1.0 (early access).** This project has not received an independent security audit. Do not use it as the only copy of important data or as the sole protection for high-value information. Read the [security limitations](#security-status-and-limitations) before use.

NEXUS-Q Vault is an open-source desktop application for protecting local files. It is designed around a simple idea: encryption and vault data should stay on the computer you control, without requiring a cloud account or a hosted service.

## At a glance

- **Local-first:** vaults, encrypted files and backups are stored at locations you choose.
- **File encryption:** encrypt individual files into `.nqx` sidecar files and restore them to a new destination.
- **Folder encryption:** protect a folder using an authenticated encrypted manifest and opaque object names; original paths are inside the encrypted manifest.
- **Recovery backup:** export an encrypted `.nqb` backup of vault/key state and restore it into a new vault with new credentials.
- **No automatic source deletion:** original files are kept; encryption does not silently remove them.
- **One cryptographic boundary:** cryptographic operations are delegated to the Rust `nexusq-core` engine.
- **Desktop experience:** Tauri 2 interface for Linux, Windows and macOS, with light/dark themes and English/Spanish UI.

## Screenshots

### Overview
<p align="center">
  <img src="docs/screenshots/overview.png" alt="NEXUS-Q Vault overview screen" width="100%">
</p>

### Create or open a vault
<p align="center">
  <img src="docs/screenshots/vault-access.png" alt="Create and open a vault screen" width="100%">
</p>

### Encrypt and restore files and folders
<p align="center">
  <img src="docs/screenshots/file-operations.png" alt="File and folder encryption operations screen" width="100%">
</p>

### Settings and local profile
<p align="center">
  <img src="docs/screenshots/settings.png" alt="Settings and local profile screen" width="100%">
</p>

## How it works

1. **Create a vault** at a location you choose and set a strong vault password.
2. **Unlock the vault** when you want to use its file-protection operations.
3. **Encrypt files or folders.** The app writes encrypted output separately and leaves source files untouched.
4. **Create a recovery backup** using a separate backup passphrase. Keep it somewhere safe and separate from the vault.
5. **Restore when needed.** Import the backup into a new vault with a new vault password.

### Important: keep all parts of your data

A `.nqb` recovery backup contains vault/key state. It does **not** include your individual `.nqx` encrypted files or the encrypted objects inside a folder vault. Keep those encrypted payloads separately, along with the backup. Losing either the payloads or the required credentials can make recovery impossible.

The backup passphrase is separate from the vault password. A backup restores vault/key state; it does not reveal or reset the original vault password.

## Current capabilities

| Area | Available in v0.1.0 |
|---|---|
| Vault lifecycle | Create, open, unlock and lock a local vault |
| Individual files | Encrypt to `.nqx`; decrypt to a new destination |
| Folders | Encrypt to an authenticated manifest plus opaque encrypted objects; restore to a new destination |
| Recovery | Export/import an encrypted `.nqb` vault/key backup |
| Password UX | Password generator and show/hide controls |
| Desktop UI | Separate navigation pages, English/Spanish labels, light/dark mode |
| Data collection | No application telemetry is implemented |

## Security status and limitations

This is an early-access prototype, **not a security-audited or production-hardened product**. The repository publishes its [threat model](THREAT_MODEL.md), [security policy](SECURITY.md), and [format/design notes](docs/DECISIONS.md), but public documentation is not a substitute for independent review.

Known limitations include:

- The app cannot protect plaintext while the vault is unlocked from malware, keyloggers, screen capture, a compromised user account or a compromised operating system.
- Original files are preserved. The app does not promise secure deletion of originals, temporary files, swap, snapshots, filesystem journals or SSD blocks.
- Folder-vault objects are currently limited to **32 MiB per file** by the folder adapter. Large-file streaming is not implemented.
- Folder encryption rejects symbolic links, omits empty directories, accepts portable UTF-8 paths only, and rejects unsafe paths and case-insensitive path collisions. Object count, approximate encrypted sizes, timestamps and access patterns remain visible.
- Folder manifest format migration/upgrade behavior is not implemented yet.
- The `.nqb` backup does not bundle encrypted file payloads or folder-vault object directories; preserve them separately.
- Filesystem permission and durability guarantees vary by platform and still need broader validation.
- Recovery usability, accessibility, update security, signing/notarization, reproducible builds and independent security review remain incomplete.
- No FIPS validation, certification, independent audit or blanket “quantum-proof” claim is made.

See [PRODUCT.md](PRODUCT.md), [THREAT_MODEL.md](THREAT_MODEL.md), [SECURITY.md](SECURITY.md), [folder format v1](docs/FOLDER_FORMAT_V1.md), and the [streaming-format proposal](docs/STREAMING_FORMAT_PROPOSAL.md) for details.

## Download

The public v0.1.0 installers are published on the [GitHub Releases page](https://github.com/lecodev-26/nexus-q-vault/releases).

| Platform | Release artifact |
|---|---|
| Linux | `.deb` and `.AppImage` |
| Windows | `.msi` (WiX) and `.exe` (NSIS installer) |
| macOS | `.dmg` |

Only download builds from this repository's Releases page. Each release includes a `SHA256SUMS.txt` file for accidental-corruption checks (checksums are not digital signatures). This early-access release may contain platform-specific issues. Code signing/notarization and independent artifact provenance are not yet provided; your operating system may show an installation warning.

On Linux, verify downloaded files with `sha256sum -c SHA256SUMS.txt`; on macOS use `shasum -a 256 -c SHA256SUMS.txt`. On Windows, compare `certutil -hashfile <installer> SHA256` with the matching entry. A matching checksum detects accidental changes but does not authenticate who built the installer.

## Build from source

### Requirements

- Rust **1.90 or newer**.
- The platform prerequisites for [Tauri 2](https://v2.tauri.app/start/prerequisites/).
- A sibling checkout of the NEXUS-Q cryptographic engine at the exact revision used by this repository.

The desktop crate uses a local path dependency on `../nexus-q`. From the Vault repository directory:

```sh
git clone https://github.com/lecodev-26/nexus-q.git ../nexus-q
git -C ../nexus-q checkout efe486603af266c379131526d8320a0584254ce4

cargo run --locked --manifest-path apps/desktop/Cargo.toml
```

Run the repository checks before submitting changes:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo fmt --manifest-path apps/desktop/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/Cargo.toml --locked --all-targets -- -D warnings
```

To build installers locally, install the Tauri CLI 2 and run:

```sh
cargo install tauri-cli --version '^2.0.0' --locked
cd apps/desktop
cargo tauri build
```

GitHub Actions validates the project and builds the Windows and macOS installers. Linux `.deb` and `.AppImage` installers are built on the project's Zorin OS 18.1 machine and uploaded to the same GitHub Release; the release checksums are regenerated to cover every platform artifact.

## Architecture

- **Desktop shell:** Tauri 2.
- **Application and filesystem boundary:** Rust commands in `apps/desktop/src/main.rs`.
- **User interface:** HTML, CSS and JavaScript in `apps/desktop/ui/`.
- **Cryptography:** `nexusq-core` from the [NEXUS-Q engine repository](https://github.com/lecodev-26/nexus-q), pinned in CI/release builds to commit `efe486603af266c379131526d8320a0584254ce4`.
- **Folder adapter:** versioned authenticated manifest and opaque encrypted objects; see [FOLDER_FORMAT_V1.md](docs/FOLDER_FORMAT_V1.md).

Vault does not implement a parallel cryptographic engine, custom key derivation or an independent encryption format.

## Project documentation

- [Product scope and MVP](PRODUCT.md)
- [Threat model](THREAT_MODEL.md)
- [Security policy and vulnerability reporting](SECURITY.md)
- [Architecture decisions](docs/DECISIONS.md)
- [Folder vault format v1](docs/FOLDER_FORMAT_V1.md)
- [Recovery design](docs/RECOVERY_DESIGN.md)
- [Streaming format proposal](docs/STREAMING_FORMAT_PROPOSAL.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [MIT license](LICENSE)

## Contributing

Issues and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first. Changes involving cryptography, key handling, recovery, file formats, backups, dependencies or release verification are security-sensitive and must include appropriate tests and threat-model updates.

## License

NEXUS-Q Vault is released under the [MIT License](LICENSE).

## Automatic locking

The desktop inactivity-lock behavior, configuration and platform limitations are documented in [docs/AUTO_LOCK.md](docs/AUTO_LOCK.md).
