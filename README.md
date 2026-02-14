# axum-ts-client

Auto-generate typed TypeScript API clients from Axum route metadata. Zero dependencies.

## What it does

You describe your API routes with a declarative Rust macro, and this crate generates a complete TypeScript client with:

- Typed fetch wrappers for every route
- Path parameter interpolation
- Query string serialization
- Auth token injection
- Error handling with typed error class
- Optional route grouping (nested objects)

## What you need

**Two crates work together:**

| Crate | What it generates | You need it for |
|---|---|---|
| **`axum-ts-client`** (this crate) | `generated.ts` — the API client with typed fetch wrappers | Route definitions, client factory, error class |
| **[`ts-rs`](https://crates.io/crates/ts-rs)** (separate) | Individual `.ts` files per type (e.g., `LoginRequest.ts`) | TypeScript type definitions for your request/response structs |

`axum-ts-client` generates `import type { LoginRequest } from "./bindings/LoginRequest"` — those files come from `ts-rs`. You need both.

## Installation

Add both to your `Cargo.toml`:

```toml
[dependencies]
axum-ts-client = "0.1"
ts-rs = { version = "11", features = ["serde-compat"] }
```

## Usage

### 1. Derive TypeScript types on your structs

Use `ts-rs` to generate `.ts` type files from your Rust types:

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
}
```

### 2. Define your route metadata

```rust
use axum_ts_client::{api_routes, RouteCollection};

pub fn routes() -> RouteCollection {
    api_routes! {
        getSession: GET "/session" [auth]
            -> SessionResponse;
        logout: POST "/logout" [auth];

        @group emailPassword

        register: POST "/register"
            body: RegisterRequest -> MessageResponse;
        login: POST "/login"
            body: LoginRequest -> LoginResponse;

        @group admin

        listUsers: GET "/admin/users" [auth]
            query: ListUsersQuery -> ListUsersResponse;
        getUser: GET "/admin/users/{id}" [auth]
            -> UserResponse;
        deleteUser: DELETE "/admin/users/{id}" [auth];
    }
}
```

### 3. Generate the client in a test

Generation runs as a `#[test]` rather than a `build.rs` script because `build.rs` runs *before* your crate compiles — it can't call your route metadata functions since they don't exist yet. A test runs *after* compilation, so it can import your crate's public API, call `all_routes()`, and pass the result to the generator.

This also means the generated file is committed to your repo, not rebuilt on every `cargo build`. You regenerate explicitly when routes change, and CI catches staleness via `check()`.

```rust
use axum_ts_client::GeneratorConfig;

fn config() -> GeneratorConfig {
    GeneratorConfig {
        bindings_dir: "./bindings".into(),
        output_path: "./packages/client/src/generated.ts".into(),
        factory_name: "createApiClient".into(),
        error_class_name: "ApiError".into(),
        options_interface_name: "ApiClientOptions".into(),
        type_import_prefix: "./bindings".into(),
        format_command: Some("bun biome check --write --unsafe".into()),
        ..Default::default()
    }
}

#[test]
fn generate_ts_client() {
    let routes = my_crate::routes();
    axum_ts_client::generate_to_file(&routes, &config()).unwrap();
}

#[test]
fn check_ts_client_up_to_date() {
    let routes = my_crate::routes();
    axum_ts_client::check(&routes, &config())
        .expect("Generated TypeScript client is out of date! Run: cargo test generate_ts_client");
}
```

Run `cargo test generate_ts_client` to regenerate. Run `check_ts_client_up_to_date` in CI to fail if someone changes routes without regenerating.

### 4. Use the generated client in TypeScript

```typescript
import { createApiClient } from "./generated";

const api = createApiClient({
  baseUrl: "http://localhost:3000",
  getToken: async () => localStorage.getItem("token"),
});

// Fully typed — args, request body, and return type are all inferred
const session = await api.getSession();
const result = await api.emailPassword.login({ email: "a@b.com", password: "secret" });
const user = await api.admin.getUser("some-id");
```

## Macro syntax

```
name: METHOD "/path" [flags]
    body: RequestType query: QueryType -> ResponseType;
```

| Element | Description |
|---|---|
| `@group name` | Groups subsequent routes into a nested object |
| `@nogroup` | Clears the current group |
| `[auth]` | Marks route as requiring authentication |
| `[redirect]` | Generates a URL builder instead of a fetch call |
| `[auth, redirect]` | Multiple flags are comma-separated |
| `body: Type` | Request body type |
| `query: Type` | Query parameters type |
| `-> Type` | Response type (omit for `void`) |
| `{param}` in path | Path parameter (becomes a `string` function arg) |

All parts except `name`, `METHOD`, `"/path"`, and the trailing `;` are optional.

## GeneratorConfig

| Field | Default | Description |
|---|---|---|
| `bindings_dir` | `"./bindings"` | Where `ts-rs` writes type files |
| `output_path` | `"./generated.ts"` | Where to write the generated client |
| `factory_name` | `"createApiClient"` | Name of the factory function |
| `enable_groups` | `true` | Nest routes into group objects |
| `error_class_name` | `"ApiError"` | Name of the generated error class |
| `options_interface_name` | `"ClientOptions"` | Name of the options interface |
| `default_credentials` | `"include"` | Default `RequestCredentials` value |
| `type_import_prefix` | (computed) | Import path from generated file to bindings dir |
| `format_command` | `None` | Shell command to format after generation (e.g., `"bun biome check --write --unsafe"`) |

## CI enforcement

The `check()` function compares the committed client file against what the generator would produce. If a route is added, removed, or changed without regenerating, `check()` fails with a clear error message.

The recommended pattern is two tests sharing a config:

```rust
#[test]
fn generate_ts_client() {
    let routes = my_crate::routes();
    axum_ts_client::generate_to_file(&routes, &config()).unwrap();
}

#[test]
fn check_ts_client_up_to_date() {
    let routes = my_crate::routes();
    axum_ts_client::check(&routes, &config())
        .expect("Generated TypeScript client is out of date! Run: cargo test generate_ts_client");
}
```

**Local dev:** run `cargo test generate_ts_client` to regenerate after changing routes.

**CI:** run `cargo test check_ts_client_up_to_date` (or just `cargo test` — both tests will run). If the committed file is stale, CI fails with:

```
Generated TypeScript client is out of sync with './packages/client/src/generated.ts'.
Run the generate command to update it.
```

If you use `format_command`, `check()` writes to a temp file, runs the formatter, and compares the formatted output against the committed file — so CI accounts for formatter changes too.

A typical CI step:

```yaml
- name: Check generated client
  run: cargo test check_ts_client_up_to_date
```

## Generated output

See [tests/snapshots/yauth_style.ts](tests/snapshots/yauth_style.ts) for a full example.

The generated client includes:
- Type imports from `ts-rs` bindings
- A typed error class (extends `Error` with `status` and `body`)
- A client options interface (`baseUrl`, `getToken`, `credentials`, `fetch`, `onError`)
- A factory function that returns an object of typed fetch methods
- A type alias: `export type ApiClient = ReturnType<typeof createApiClient>`

## License

MIT
