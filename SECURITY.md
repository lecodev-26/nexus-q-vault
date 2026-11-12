# Security Policy

## Status
NEXUS-Q Vault is an early-access project, not a security-audited or production-hardened product. It has not received an independent security audit. Do not rely on it as the sole protection for high-value or irreplaceable data.

## Reporting a vulnerability
Do not publish exploitable details in public issues. Until a dedicated private advisory channel and response team are established, contact maintainers through the private vulnerability reporting feature of the repository host (GitHub Security Advisories, when enabled). If unavailable, request a private contact route without including exploit details in a public message.

Include affected version or commit, platform, impact, reproduction steps and suggested mitigation. Do not attach real secrets or personal files.

## Response targets
Initial targets, not a 24/7 guarantee:
- Acknowledge within 7 calendar days.
- Triage severity and affected versions within 14 calendar days.
- Coordinate a fix, disclosure timeline and credit with the reporter.
- Publish an advisory and fixed release when users can reasonably update.

## Supported versions
This is the first public release. Support is best-effort; there are no production-supported versions or guaranteed security response SLAs.

## Security change control
Changes to cryptography, key lifecycle, recovery, file parsing, vault format, backup/restore, dependencies and update verification require security review. Document threat-model impact and add regression tests. Never claim an audit, certification, reproducible build or signature verification unless evidence is public and independently checkable.

## Known limitations
The app cannot protect plaintext while open against a compromised OS, malware running as the user, keyloggers or screen capture. Deleting a source file is not equivalent to secure erasure. Backups and recovery material are sensitive and must be handled separately from the vault password.
