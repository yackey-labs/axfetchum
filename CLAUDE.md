# CLAUDE.md — axfetchum

## What This Is

`axfetchum` is a zero-dependency Rust crate that auto-generates typed TypeScript API clients from Axum route metadata. It combines a declarative `api_routes!` macro with a code generator to produce complete, typed fetch wrappers.

**Repo:** `github.com/yackey-labs/axfetchum`

## Key Commands

```bash
cargo test                    # Run all tests (41)
cargo clippy -- -D warnings   # Lint
cargo fmt --check             # Format check
```

## Architecture

### Files

| File | Purpose |
|---|---|
| `src/types.rs` | `RouteDefinition`, `HttpMethod`, `PathParam`, `RouteCollection` |
| `src/macros.rs` | `api_routes!` declarative macro |
| `src/generator.rs` | TypeScript client code generator + `check()` mode |
| `src/lib.rs` | Public API re-exports |
| `tests/macro_tests.rs` | Macro expansion tests |
| `tests/generator_tests.rs` | Generator output + snapshot tests |
| `tests/snapshots/` | Generated TS output for visual inspection |

### How Consumers Use It

1. Add `#[derive(TS)] #[ts(export)]` to request/response types (via `ts-rs` crate)
2. Define route metadata using `api_routes!` macro
3. Call `generate_to_file()` in a test to produce the TypeScript client
4. Call `check()` in CI to detect stale generated files

### Zero Dependencies

This crate uses only `std` — no `syn`, `quote`, `proc-macro2`. The macro uses `stringify!()` for type names, avoiding type resolution at compile time.

## Versioning

- **Semantic versioning** is automated via [knope](https://knope.tech) + Forgejo CI
- **NEVER manually edit version numbers** in `Cargo.toml` — knope manages them from conventional commits
- `feat:` → minor bump, `fix:` → patch bump, `feat!:` / `fix!:` / `BREAKING CHANGE:` → major bump
- Pushing to `main` triggers: CI check → `knope release` → version bump + changelog + GitHub release + `cargo publish`
- The `chore: prepare release` commit pushed by the release job is skipped by the `if: !startsWith(...)` guard
- To preview what knope will do: `knope release --dry-run`

## Conventions

- **Always use Bun** when JS tooling is needed — never npm or yarn
- **Conventional commits** for all commit messages — this directly drives automated versioning
- All public API is re-exported from `lib.rs`
- Snapshot tests write to `tests/snapshots/` for visual verification
