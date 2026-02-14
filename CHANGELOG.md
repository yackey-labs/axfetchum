## 0.1.4 (2026-02-14)

### Fixes

- publish crate before git push to prevent cancellation
- add Cargo.lock to versioned_files and --allow-dirty

## 0.1.3 (2026-02-14)

### Features

- add format_command to GeneratorConfig

## 0.1.2 (2026-02-14)

### Fixes

- add CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS for cargo publish

## 0.1.1 (2026-02-14)

### Features

- initial axum-ts-client crate
- add automated semver releases via knope + Forgejo CI

### Fixes

- use --strip-components=1 for knope tarball extraction
