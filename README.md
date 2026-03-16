# axum-ts-client

Auto-generate typed TypeScript API clients from Axum route metadata.

## Quick start

Define your routes once — get both an Axum router and a typed TypeScript client:

```rust
use axum_ts_client::ApiRouter;

let (router, routes) = ApiRouter::<AppState>::new()
    .group("emailPassword")
    .post("/register", register)
        .json::<RegisterRequest, MessageResponse>()
        .done()
    .post("/login", login)
        .body::<LoginRequest>()
        .done()
    .post("/change-password", change_password)
        .json::<ChangePasswordRequest, MessageResponse>()
        .auth()
        .done()
    .group("admin")
    .get("/admin/users", list_users)
        .response::<Vec<UserResponse>>()
        .auth()
        .done()
    .get("/admin/users/{id}", get_user)
        .response::<UserResponse>()
        .auth()
        .done()
    .delete("/admin/users/{id}", delete_user)
        .auth()
        .done()
    .build();

// `router` is a real Axum Router — plug it into your app
// `routes` generates a TypeScript client like this:
```

```typescript
const api = createApiClient({
  baseUrl: "http://localhost:3000",
  getToken: async () => localStorage.getItem("token"),
});

await api.emailPassword.register({ email: "a@b.com", password: "secret" });
await api.emailPassword.login({ email: "a@b.com", password: "secret" });
const users = await api.admin.listUsers();
const user = await api.admin.getUser("some-id");
```

Handler names auto-convert to camelCase: `list_users` → `listUsers`, `change_password` → `changePassword`. Override with `.as_("customName")` when needed.

## Installation

```toml
[dependencies]
axum-ts-client = { version = "0.1", features = ["axum"] }
ts-rs = { version = "11", features = ["serde-compat"] }
```

The `axum` feature enables the `ApiRouter` builder. Without it, the crate is zero-dependency and provides just the `api_routes!` macro and code generator.

## How it works

**Two crates work together:**

| Crate | Generates | Purpose |
|---|---|---|
| **`axum-ts-client`** | `generated.ts` — typed fetch wrappers | Route definitions, client factory, error handling |
| **[`ts-rs`](https://crates.io/crates/ts-rs)** | Individual `.ts` type files | TypeScript interfaces for your Rust structs |

`axum-ts-client` generates `import type { LoginRequest } from "./bindings/LoginRequest"` — those files come from `ts-rs`.

## Two ways to define routes

### Option A: ApiRouter builder (recommended)

One definition, zero duplication — builds both the Axum router and the route metadata:

```rust
use axum_ts_client::ApiRouter;

fn user_routes() -> (Router<AppState>, RouteCollection) {
    ApiRouter::<AppState>::new()
        .group("users")
        .get("/users", list_users)
            .response::<Vec<UserResponse>>()
            .auth()
            .done()
        .post("/users", create_user)
            .json::<CreateUserRequest, UserResponse>()
            .auth()
            .done()
        .get("/users/{id}", get_user)
            .response::<UserResponse>()
            .auth()
            .done()
        .delete("/users/{id}", delete_user)
            .auth()
            .done()
        .build()
}
```

**Builder features:**
- **Auto camelCase names** — `list_users` → `listUsers`, no string literals needed
- **`.json::<Body, Response>()`** — set both types in one call
- **`.done()`** — finalize with the auto-derived name
- **`.as_("name")`** — override the auto-name when you want something different
- **`.merge()`** — compose multiple ApiRouters (great for plugin architectures)
- **`Vec<T>`, `Option<T>`** — generic types just work

Requires `features = ["axum"]`.

### Option B: Declarative macro (zero dependencies)

Metadata-only, no Axum dependency — you build the router separately:

```rust
use axum_ts_client::{api_routes, RouteCollection};

pub fn routes() -> RouteCollection {
    api_routes! {
        @group emailPassword

        register: POST "/register"
            body: RegisterRequest -> MessageResponse;
        login: POST "/login"
            body: LoginRequest -> LoginResponse;
        listUsers: GET "/admin/users" [auth]
            query: ListUsersQuery -> Vec<UserResponse>;
        getUser: GET "/admin/users/{id}" [auth]
            -> UserResponse;
        authorize: GET "/oauth/{provider}/authorize" [redirect]
            query: AuthorizeQuery;
    }
}
```

**Macro syntax:**

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
| `body: Type` | Request body (supports `Vec<T>`, `Option<T>`) |
| `query: Type` | Query parameters (supports `Vec<T>`, `Option<T>`) |
| `-> Type` | Response type (supports `Vec<T>`, `Option<T>`, omit for `void`) |
| `{param}` in path | Path parameter (becomes a `string` function arg) |

## Generating the client

Generation runs as a `#[test]` — not a `build.rs` — because it needs to call your crate's route functions after compilation.

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
    // With ApiRouter:
    let (_router, routes) = my_app::user_routes();
    // Or with macro:
    // let routes = my_app::routes();
    axum_ts_client::generate_to_file(&routes, &config()).unwrap();
}

#[test]
fn check_ts_client_up_to_date() {
    let (_router, routes) = my_app::user_routes();
    axum_ts_client::check(&routes, &config())
        .expect("Generated TypeScript client is out of date! Run: cargo test generate_ts_client");
}
```

**Local:** `cargo test generate_ts_client` to regenerate.
**CI:** `cargo test check_ts_client_up_to_date` fails if the committed file is stale.

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
| `format_command` | `None` | Shell command to format after generation |

## Generated output

The generated client includes:
- Type imports from `ts-rs` bindings
- A typed error class (extends `Error` with `status` and `body`)
- A client options interface (`baseUrl`, `getToken`, `credentials`, `fetch`, `onError`)
- A factory function returning typed fetch methods with optional grouping
- A type alias: `export type ApiClient = ReturnType<typeof createApiClient>`

See [tests/snapshots/yauth_style.ts](tests/snapshots/yauth_style.ts) for a full example.

## License

MIT
