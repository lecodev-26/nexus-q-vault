# Shared file exchange v1 — draft for security review

**Status:** proposed implementation draft; not independently security-reviewed or finalized.

## Protocol

A `.nqcontact` file is UTF-8 JSON with `format=nexusq-contact-card`, `version=1`, `identity_id`, display `label`, `key_id`, `algorithm=mlkem768-xwing-v1`, `public_key` (JSON byte array), and `fingerprint`.

The fingerprint is SHA-256 over the domain separator `NEXUS-Q CONTACT CARD V1\0` and length-prefixed identity ID, label, key ID and public key. It is not a signature. The UI requires the user to compare the full fingerprint with the recipient through a separate trusted channel before saving the contact as verified. The checkbox records the user's local attestation; it does not authenticate the card issuer.

The sender's verified-contact list is stored locally in the desktop webview's browser storage. It contains public contact-card data only, is not synchronized, and is not included in `.nqb` backups. Anyone with access to the same local application profile may be able to inspect it.

A shared `.nqx` package reuses the existing Core public-key envelope v1 (CBOR), hybrid ML-KEM-768 + X25519 encapsulation and AES-256-GCM payload protection. Envelope metadata is empty so filenames and paths are not disclosed. The format does not authenticate the sender.

## Key lifecycle and recovery

Each Vault receiving identity has a dedicated `MlKem768` / `KeyAgreement` key. Its private seed remains wrapped under the Vault KEK; the UI sees only the public half. `.nqb` backups include the identity-to-key reference and wrapped key material, re-wrapped under the new vault password during restore. The separate backup passphrase is required. The `.nqb` does not include `.nqx` packages or the local sender contact list.

A new receiving identity means a new contact card and fingerprint. Old active/rotating/retired keys remain usable for opening packages while retained; revoked/destroyed keys do not. Do not destroy old keys while packages addressed to them may still be needed.

Receiving-key creation generates the key before creating its identity. If identity creation fails, the desktop app attempts Session::destroy_key immediately; on success, the key material is destroyed and its record remains as a destroyed lifecycle entry. If Core policy or audit failure prevents cleanup, the UI error includes the key ID and instructs the user to reopen the vault and inspect that record: an audit-write failure may occur after the in-memory status changed but before it was persisted. If the record is still generated, retry Core's explicit key-destruction operation after resolving the policy/audit issue. The desktop UI does not expose a separate orphan-key recovery screen. Failures after identity creation but before the identity/key relationship is fully validated can still leave an incomplete identity or inactive key; full grouped rollback is not provided by Core.

## Limits and security properties

- One file and one recipient per package; plaintext limit 32 MiB, package overhead allowance 16 KiB.
- Empty files are supported. Inputs are bounded; Unix opens reject symbolic links and use nonblocking mode so named pipes cannot stall the reader. Windows opens reparse points directly and reject any handle carrying FILE_ATTRIBUTE_REPARSE_POINT, including file and directory symlinks.
- Output is atomically published without overwriting existing destinations. Originals are never modified. Temporary output files are created with owner-only permissions on Unix and a protected owner-only DACL on Windows before plaintext is written. A crash can still leave an owner-readable temporary file; secure erasure is not guaranteed.
- Malformed, truncated, unsupported-version/mode, wrong-recipient and tampered packages are rejected by Core.
- Successful decryption proves only that the selected receiving key opened the package and the AEAD tag was valid. It does not identify the sender.
- No server, sender signature, replay prevention, delivery receipt, multi-recipient support or forward secrecy against later compromise of the recipient's long-term key.
- If the receiving key and all usable `.nqb` backups are lost, packages addressed to it cannot be recovered.

## Review required before stabilization

Independently review the fingerprint construction and verification UX, Core envelope reuse, key lifecycle and backup behavior, metadata visibility, filesystem race/atomic-write behavior, key rotation/revocation and cross-platform support. Regression tests cover Unix mode 0600, Windows protected owner-only DACLs, Unix named-pipe rejection, and Windows file/directory reparse-point rejection; the Windows tests require the CI runner to permit creating symbolic links. The workflow best-effort destroys a generated key if identity creation fails, but full grouped rollback is still unavailable: failures after identity creation can leave an incomplete identity or inactive key in the unlocked session. Failures in the Windows ACL/reparse-point tests require Windows CI/manual investigation; if a platform runner cannot create symlinks, the reparse-point case remains a manual validation requirement on that platform. This draft is not a security certification.
