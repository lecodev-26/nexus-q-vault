# Architecture decisions

## ADR-001 — One cryptographic engine
**Status:** accepted for the alpha; supply-chain review remains open.

NEXUS-Q Vault delegates cryptographic operations, vault container handling, key lifecycle and backup encryption to `nexusq-core`. The app must not add a second crypto provider or custom file encryption format. A narrow Rust application layer translates user intent into core API calls.

The desktop crate uses a sibling source checkout through a Cargo path dependency for local builds. CI and release workflows fetch the engine at immutable commit `458fc393992f4b8030ff594a408620058032e214`, and the desktop lockfile is committed. The CI and release workflows must use this same revision. This pins the source revision used by those workflows; independent provenance verification, signed build attestations and a reviewed published engine dependency remain open supply-chain work.

## ADR-002 — Local-first MVP
**Status:** accepted. No account, cloud, synchronization or hosted network service is required. Recipient-encrypted file exchange is a local workflow and does not require a hosted service.

## ADR-003 — Preserve source files
**Status:** accepted for MVP. Encryption never deletes the original automatically. Secure deletion cannot be guaranteed on modern filesystems and SSDs. Any future cleanup action must explain this limitation and require confirmation.

## ADR-004 — MIT license
**Status:** accepted for the public alpha. MIT is simple and permissive. Any future license change requires explicit project governance and cannot change licenses or notices required by dependencies.

## ADR-005 — Do not paper over current file I/O limitations
**Status:** accepted for the integration phase; release gate remains open.

The pinned `nexusq-core` revision (`458fc393992f4b8030ff594a408620058032e214`) still loads complete inputs and envelopes into memory. Its file helpers now write to exclusive same-directory temporary files, sync file contents, and publish with a hard link that fails if the destination already exists. On Unix, the temporary file is created with mode `0600` and the parent directory is synced. This provides atomic create-only publication, not atomic replacement. Non-Unix permission and durability semantics need platform validation.

NEXUS-Q Vault will keep using the engine as its only cryptographic implementation. It will not introduce a parallel envelope format or home-grown streaming cipher to compensate. Before beta, the integration still needs bounded resource use, explicit platform guarantees, overwrite UX/policy, and broader filesystem fault-injection tests. The upstream implementation is pinned to `nexus-q` commit `458fc393992f4b8030ff594a408620058032e214`; it is not an independent security audit.

The application-level tests cover wrong vault passwords, successful byte-for-byte file and backup roundtrips, recovery into a new vault password, folder-vault restoration after backup recovery, folder roundtrips, path traversal rejection, symlink rejection, opaque object names, and tampered manifest/object rejection without leaving partial restore output. These tests do not establish crash consistency under power loss, large-file safety, cross-platform filesystem guarantees, or independent security review.

## ADR-006 — Tauri desktop boundary
**Status:** accepted for the desktop prototype; security review pending.

The cross-platform desktop shell uses Tauri 2 with a Rust command layer. The frontend handles presentation and user input only; it does not implement or select cryptographic algorithms. All vault, file-envelope and backup operations call `nexusq-core` directly. The unlocked `Session` stays in Rust process state behind a mutex and is never serialized to the webview. Password inputs are transferred only for the operation and wrapped in zeroizing byte buffers on the Rust side. Password fields are cleared after each submitted attempt and when the vault is locked. JavaScript strings cannot be reliably zeroized and may remain transiently in the webview runtime until garbage collection; the app does not persist them.

The initial folder format is documented in [FOLDER_FORMAT_V1.md](FOLDER_FORMAT_V1.md). The prototype uses native file/folder pickers via `tauri-plugin-dialog` and still permits manual path editing. Native installer bundling is enabled for the public release (Linux `.deb`/AppImage built on the project's Zorin OS 18.1 host and uploaded manually; Windows MSI/NSIS and macOS DMG built by GitHub Actions). The v0.2.0 release candidate uses SemVer (`0.2.0`); the Windows WiX MSI uses the separate numeric `bundle.windows.wix.version` (`0.2.0.1`) required by MSI ProductVersion. This Windows-specific installer version is an internal packaging constraint, not the application version. Code signing, notarization and broader platform validation remain open. Individual files can be encrypted to sidecar `.nqx` envelopes. Folder encryption stores encrypted objects under opaque `nqk_file_*` identifiers and publishes a version-1 `manifest.nqm` envelope last; filenames, relative paths and plaintext sizes are inside the authenticated manifest. The manifest envelope uses a fixed public metadata label and no filenames. Each object envelope authenticates its opaque object identifier in public metadata; restore verifies this binding so swapping two ciphertext objects is rejected. Symlinks and non-portable paths are rejected; empty directories are omitted; each file is capped at 32 MiB while the engine remains in-memory. Folder object files and `.nqx` sidecars are not included in the `.nqb` vault-key backup and must be backed up separately. Dialog cancellation behavior, close/lock lifecycle, platform-specific permission semantics, dependency provenance, installer signing and accessibility remain open work. This ADR does not make the prototype production-ready.

# ADR-007 — Native desktop file and folder dialogs
**Status:** accepted for the desktop prototype; platform review pending.

Use the official Tauri 2 dialog plugin through its Rust API. The frontend invokes narrow Rust commands for selecting files, save destinations and folders; it does not use a JavaScript filesystem API or read file contents. File dialogs filter known `.nqv`, `.nqb` and `.nqx` formats where appropriate. Folder-vault creation and restore dialogs select a parent directory and propose a new child directory, whose path remains editable.

The dialog calls are exposed as asynchronous Tauri commands using the plugin's blocking native-dialog methods off the UI thread. Cancel returns no selection and leaves the existing path unchanged. The selected local path is returned to the frontend for display and then passed back to the relevant Rust operation. Non-UTF-8 paths are rejected explicitly rather than converted lossily. This does not establish a sandbox or prevent access to other local files; the app remains a user-authorized local desktop application. Cross-platform cancellation, permission and accessibility behavior must be tested before beta.


## ADR-008 — Streaming file envelope NQX2
**Status:** proposed only; no implementation approved.

NQX1 remains the existing single-payload envelope and must stay readable. The desktop app must not implement streaming cryptography. A future chunked NQX2 format belongs in nexus-q and must reuse the existing KeyRecord wrapping hierarchy and AEAD primitives; no new crypto provider, KDF or custom cipher is permitted.

The current proposal uses bounded chunks, per-file random DEKs, nonce-prefix plus monotonic chunk indices, domain-separated AAD binding the exact serialized header and chunk index, and a mandatory authenticated footer to detect truncation and authenticate empty files. Decryption writes only to a private temporary file and publishes create-only after every chunk, footer and EOF check succeeds. Folder object metadata must remain bound to opaque object IDs to prevent ciphertext swapping.

Chunk size and maximum file size in the proposal are provisional. Nonce discipline, per-key usage limits, footer semantics, crash consistency, parser bounds and cross-platform filesystem behavior require engine-level review, test vectors, fuzzing and fault injection before implementation. NQX1 files are never rewritten in place; migration writes a new file and preserves the original. See [STREAMING_FORMAT_PROPOSAL.md](STREAMING_FORMAT_PROPOSAL.md). This ADR does not authorize changes to the v4.1.2 release or its artifacts.
