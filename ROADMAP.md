# Roadmap

- [x] Foundation: repository structure, license, governance and initial threat model.
- [x] MVP definition and nexusq-core integration spike.
- [x] Public repository and contribution templates.
- [ ] Security-reviewed vault/file/backup adapter and automated integration tests.
- [ ] Recovery design and user-tested recovery flow.
  - [x] Draft backup-based recovery proposal and automated restore of file and folder vaults with a new vault password (user testing still pending).
- [ ] Encrypted folder manifest and format migration policy.
  - [x] Version-1 encrypted manifest, opaque object names, safe relative paths, atomic create-only publication and folder roundtrip tests.
  - [ ] Format migration, empty-directory preservation, large-file streaming and cross-platform filesystem review.
    - [x] Drafted the NQX2 chunked-streaming proposal; no cryptographic code is implemented pending core review.
    - [ ] Review and implement the format in nexus-q with NQX1 backward compatibility and fuzz/fault-injection tests.
- [ ] Desktop UI and accessible onboarding.
  - [x] Initial Tauri 2 desktop shell and Rust command boundary (prototype only; not yet user-tested).
  - [x] UI commands for individual files, folder encryption/restoration and encrypted key backups.
  - [x] Native file/folder pickers through Tauri's cross-platform dialog plugin.
  - [x] Initial accessibility pass: semantic status/errors, labelled picker controls, busy states, keyboard focus and reduced-motion support.
  - [x] Password visibility controls; visibility resets when forms clear secrets or the vault is locked.
  - [x] Configurable automatic inactivity/system-event locking and recovery-safe lock failure handling (PRs #25–#26).
  - [x] Cancellable operations center (PR #27).
  - [x] Recipient-encrypted file exchange with file-handling hardening (PRs #29–#30).
  - [ ] User testing, onboarding polish and full accessibility review.
- [ ] Internationalization and community translations.
  - [x] Establish a maintainable localization structure, language selection and English fallback.
  - [x] Add Catalan (ca) alongside the existing English and Spanish interface (PR #28).
  - [x] Add French (fr), German (de), Portuguese (Portugal, pt-PT) and Portuguese (Brazil, pt-BR) (PRs #31–#33).
  - [ ] Add Italian (it) after a reviewed translation is available.
  - [ ] Expand to Japanese (ja), Korean (ko), Simplified Chinese (zh-CN) and Traditional Chinese (zh-TW).
  - [ ] Add Arabic (ar), Hindi (hi), Indonesian (id), Turkish (tr), Russian (ru) and other community-requested languages.
  - [ ] Establish translation review guidance so contributed translations are checked for accuracy, consistency, accessibility and security-sensitive terminology.
- [x] CI for library and desktop checks/builds; cross-platform public installer release workflow.
- [ ] Dependency review, fuzzing, signed releases and reproducible-build investigation.
- [ ] External security review and community beta.
- [ ] 1.0 release after recovery, format and update-security criteria are met.

Language order is a staged plan, not a promise that every language will ship in a specific release. Priorities may change based on community demand and the availability of reviewers fluent in each language. English remains the fallback until a translation is complete and reviewed.

A roadmap item is not complete merely because a prototype exists. Security milestones require published evidence and review.
