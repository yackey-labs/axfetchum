use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::path::Path;

use crate::types::{HttpMethod, RouteCollection, RouteDefinition};

/// Configuration for the TypeScript client generator.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Directory where ts-rs generates type bindings (e.g., `"./bindings"`).
    pub bindings_dir: String,
    /// Output path for the generated client file.
    pub output_path: String,
    /// Name of the factory function (e.g., `"createYAuthClient"`).
    pub factory_name: String,
    /// Whether to generate grouped nested objects.
    pub enable_groups: bool,
    /// Name of the error class (e.g., `"ApiError"` or `"YAuthError"`).
    pub error_class_name: String,
    /// Name of the options interface (e.g., `"ClientOptions"` or `"YAuthClientOptions"`).
    pub options_interface_name: String,
    /// Whether to include credentials by default.
    pub default_credentials: String,
    /// Import path prefix for types (relative from generated file to bindings dir).
    /// If empty, computed from bindings_dir relative to output_path.
    pub type_import_prefix: String,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            bindings_dir: "./bindings".into(),
            output_path: "./generated.ts".into(),
            factory_name: "createApiClient".into(),
            enable_groups: true,
            error_class_name: "ApiError".into(),
            options_interface_name: "ClientOptions".into(),
            default_credentials: "include".into(),
            type_import_prefix: String::new(),
        }
    }
}

/// Error returned by the check function when generated output doesn't match committed file.
#[derive(Debug)]
pub enum CheckError {
    /// Generated output differs from the committed file.
    OutOfSync { path: String },
    /// Could not read the committed file.
    ReadError { path: String, error: std::io::Error },
    /// Generation itself failed.
    GenerateError(String),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfSync { path } => {
                write!(
                    f,
                    "Generated TypeScript client is out of sync with '{path}'. \
                     Run the generate command to update it."
                )
            }
            Self::ReadError { path, error } => {
                write!(f, "Failed to read '{path}': {error}")
            }
            Self::GenerateError(msg) => write!(f, "Generation error: {msg}"),
        }
    }
}

impl std::error::Error for CheckError {}

/// Collect all unique type names referenced in the routes.
fn collect_type_names(routes: &RouteCollection) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    for route in routes {
        if let Some(ref t) = route.body_type {
            types.insert(t.clone());
        }
        if let Some(ref t) = route.response_type {
            types.insert(t.clone());
        }
        if let Some(ref t) = route.query_type {
            types.insert(t.clone());
        }
    }
    types
}

/// Compute the import path prefix for type imports.
fn compute_import_prefix(config: &GeneratorConfig) -> String {
    if !config.type_import_prefix.is_empty() {
        return config.type_import_prefix.clone();
    }

    // Compute relative path from output file to bindings dir
    let output_dir = Path::new(&config.output_path)
        .parent()
        .unwrap_or(Path::new("."));
    let bindings = Path::new(&config.bindings_dir);

    // Simple relative path computation:
    // If output is at ./packages/client/src/generated.ts and bindings at ./bindings,
    // we need ../../../bindings
    // For simplicity, if both are relative, compute manually
    let output_str = output_dir.to_string_lossy();
    let bindings_str = bindings.to_string_lossy();

    // Strip leading ./ for comparison
    let output_clean = output_str.strip_prefix("./").unwrap_or(&output_str);
    let bindings_clean = bindings_str.strip_prefix("./").unwrap_or(&bindings_str);

    // Count depth of output directory
    let output_parts: Vec<&str> = if output_clean.is_empty() {
        vec![]
    } else {
        output_clean.split('/').collect()
    };

    // Find common prefix
    let binding_parts: Vec<&str> = if bindings_clean.is_empty() {
        vec![]
    } else {
        bindings_clean.split('/').collect()
    };

    let mut common = 0;
    for (a, b) in output_parts.iter().zip(binding_parts.iter()) {
        if a == b {
            common += 1;
        } else {
            break;
        }
    }

    let ups = output_parts.len() - common;
    let remaining = &binding_parts[common..];

    let mut prefix = String::new();
    if ups == 0 {
        prefix.push_str("./");
    } else {
        for _ in 0..ups {
            prefix.push_str("../");
        }
    }
    prefix.push_str(&remaining.join("/"));

    prefix
}

/// Convert a Rust type name (from stringify!) to a TypeScript type string.
///
/// This handles the route signature types only — the actual type definitions
/// are generated by ts-rs separately.
fn rust_type_to_ts(rust_type: &str) -> String {
    let trimmed = rust_type.trim();

    // Handle Vec<T> -> T[]
    if let Some(inner) = trimmed
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix('>'))
    {
        let inner_ts = rust_type_to_ts(inner);
        return format!("{inner_ts}[]");
    }

    // Handle Option<T> -> T | null
    if let Some(inner) = trimmed
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        let inner_ts = rust_type_to_ts(inner);
        return format!("{inner_ts} | null");
    }

    // Primitive mappings
    match trimmed {
        "String" | "&str" | "Uuid" => "string".into(),
        "bool" => "boolean".into(),
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "usize"
        | "isize" => "number".into(),
        _ => trimmed.to_string(),
    }
}

/// Check if a type is a primitive (doesn't need an import).
fn is_primitive_type(rust_type: &str) -> bool {
    let trimmed = rust_type.trim();

    // Unwrap Vec/Option
    if let Some(inner) = trimmed
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return is_primitive_type(inner);
    }
    if let Some(inner) = trimmed
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return is_primitive_type(inner);
    }

    matches!(
        trimmed,
        "String"
            | "&str"
            | "Uuid"
            | "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "f32"
            | "f64"
            | "usize"
            | "isize"
    )
}

/// Extract the base custom type name for imports (unwrapping Vec/Option).
fn base_type_name(rust_type: &str) -> Option<String> {
    let trimmed = rust_type.trim();

    if let Some(inner) = trimmed
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return base_type_name(inner);
    }
    if let Some(inner) = trimmed
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return base_type_name(inner);
    }

    if is_primitive_type(trimmed) {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Build the path template string for TypeScript.
///
/// `/admin/users/{id}` -> `` `/admin/users/${id}` ``
fn build_path_template(path: &str) -> String {
    if !path.contains('{') {
        return format!("\"{path}\"");
    }

    // Replace {param} with ${param} for template literals
    let mut template = String::new();
    for ch in path.chars() {
        if ch == '{' {
            template.push_str("${");
        } else {
            template.push(ch);
        }
    }

    format!("`{template}`")
}

/// Generate a function's parameter list for a route.
fn generate_params(route: &RouteDefinition) -> String {
    let mut params = Vec::new();

    // Path params come first
    for param in &route.path_params {
        params.push(format!("{}: string", param.name));
    }

    // Body param
    if let Some(ref body_type) = route.body_type {
        let ts_type = rust_type_to_ts(body_type);
        params.push(format!("body: {ts_type}"));
    }

    // Query param
    if let Some(ref query_type) = route.query_type {
        let ts_type = rust_type_to_ts(query_type);
        params.push(format!("query?: {ts_type}"));
    }

    params.join(", ")
}

/// Generate the request options object literal for a route.
fn generate_request_options(route: &RouteDefinition) -> String {
    let mut opts = Vec::new();

    if route.method != HttpMethod::Get {
        opts.push(format!("method: \"{}\"", route.method.as_str()));
    }

    if route.auth {
        opts.push("auth: true".into());
    }

    if route.body_type.is_some() {
        opts.push("body".into());
    }

    if route.query_type.is_some() {
        opts.push("query".into());
    }

    if opts.is_empty() {
        String::new()
    } else {
        format!(", {{ {} }}", opts.join(", "))
    }
}

/// Generate the full TypeScript client source code.
pub fn generate(routes: &RouteCollection, config: &GeneratorConfig) -> String {
    let mut out = String::new();

    // Header
    writeln!(out, "// Auto-generated by axum-ts-client. Do not edit.").unwrap();
    writeln!(out).unwrap();

    // Collect and generate type imports
    let type_names = collect_type_names(routes);
    let import_prefix = compute_import_prefix(config);

    let custom_types: BTreeSet<String> = type_names
        .iter()
        .filter_map(|t| base_type_name(t))
        .collect();

    for type_name in &custom_types {
        writeln!(
            out,
            "import type {{ {type_name} }} from \"{import_prefix}/{type_name}\";"
        )
        .unwrap();
    }

    if !custom_types.is_empty() {
        writeln!(out).unwrap();
    }

    // Error class
    let error_name = &config.error_class_name;
    writeln!(out, "export class {error_name} extends Error {{").unwrap();
    writeln!(
        out,
        "  constructor(message: string, public status: number, public body?: unknown) {{"
    )
    .unwrap();
    writeln!(out, "    super(message);").unwrap();
    writeln!(out, "    this.name = \"{error_name}\";").unwrap();
    writeln!(out, "  }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Options interface
    let opts_name = &config.options_interface_name;
    writeln!(out, "export interface {opts_name} {{").unwrap();
    writeln!(out, "  baseUrl: string;").unwrap();
    writeln!(out, "  getToken?: () => Promise<string | null>;").unwrap();
    writeln!(out, "  credentials?: RequestCredentials;").unwrap();
    writeln!(out, "  fetch?: typeof fetch;").unwrap();
    writeln!(out, "  onError?: (error: {error_name}) => void;").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Internal request options type
    writeln!(out, "type RequestOptions = {{").unwrap();
    writeln!(out, "  method?: string;").unwrap();
    writeln!(out, "  body?: unknown;").unwrap();
    writeln!(out, "  query?: Record<string, unknown>;").unwrap();
    writeln!(out, "  auth?: boolean;").unwrap();
    writeln!(out, "}};").unwrap();
    writeln!(out).unwrap();

    // Request helper
    generate_request_helper(&mut out, config);

    // Factory function
    let factory = &config.factory_name;
    writeln!(out, "export function {factory}(options: {opts_name}) {{").unwrap();
    writeln!(out, "  const request = createRequest(options);").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "  return {{").unwrap();

    if config.enable_groups {
        generate_grouped_routes(&mut out, routes);
    } else {
        generate_flat_routes(&mut out, routes);
    }

    writeln!(out, "  }};").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Type export
    writeln!(
        out,
        "export type {} = ReturnType<typeof {}>;",
        derive_type_name(factory),
        factory
    )
    .unwrap();

    out
}

/// Generate the internal `createRequest` helper function.
fn generate_request_helper(out: &mut String, config: &GeneratorConfig) {
    let error_name = &config.error_class_name;
    let opts_name = &config.options_interface_name;
    let default_creds = &config.default_credentials;

    writeln!(out, "function createRequest(options: {opts_name}) {{").unwrap();
    writeln!(
        out,
        "  const {{ baseUrl, credentials = \"{default_creds}\" }} = options;"
    )
    .unwrap();
    writeln!(out, "  const fetchFn = options.fetch ?? globalThis.fetch;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "  async function request<T>(").unwrap();
    writeln!(out, "    path: string,").unwrap();
    writeln!(out, "    opts: RequestOptions = {{}},").unwrap();
    writeln!(out, "  ): Promise<T> {{").unwrap();
    writeln!(
        out,
        "    const {{ method = \"GET\", body, query, auth }} = opts;"
    )
    .unwrap();
    writeln!(out).unwrap();

    // Build URL with query params
    writeln!(out, "    let url = `${{baseUrl}}${{path}}`;").unwrap();
    writeln!(out, "    if (query) {{").unwrap();
    writeln!(out, "      const params = new URLSearchParams();").unwrap();
    writeln!(
        out,
        "      for (const [key, value] of Object.entries(query)) {{"
    )
    .unwrap();
    writeln!(out, "        if (value !== undefined && value !== null) {{").unwrap();
    writeln!(out, "          params.set(key, String(value));").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "      }}").unwrap();
    writeln!(out, "      const qs = params.toString();").unwrap();
    writeln!(out, "      if (qs) url += `?${{qs}}`;").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    // Build headers
    writeln!(
        out,
        "    const headers: Record<string, string> = {{ \"Content-Type\": \"application/json\" }};"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    if (auth && options.getToken) {{").unwrap();
    writeln!(out, "      const token = await options.getToken();").unwrap();
    writeln!(
        out,
        "      if (token) headers[\"Authorization\"] = `Bearer ${{token}}`;"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    // Fetch
    writeln!(out, "    const response = await fetchFn(url, {{").unwrap();
    writeln!(out, "      method,").unwrap();
    writeln!(out, "      credentials,").unwrap();
    writeln!(out, "      headers,").unwrap();
    writeln!(out, "      body: body ? JSON.stringify(body) : undefined,").unwrap();
    writeln!(out, "    }});").unwrap();
    writeln!(out).unwrap();

    // Error handling
    writeln!(out, "    if (!response.ok) {{").unwrap();
    writeln!(out, "      const text = await response.text();").unwrap();
    writeln!(out, "      let message: string;").unwrap();
    writeln!(out, "      let errorBody: unknown;").unwrap();
    writeln!(out, "      try {{").unwrap();
    writeln!(out, "        const json = JSON.parse(text);").unwrap();
    writeln!(out, "        message = json.error ?? json.message ?? text;").unwrap();
    writeln!(out, "        errorBody = json;").unwrap();
    writeln!(out, "      }} catch {{").unwrap();
    writeln!(out, "        message = text;").unwrap();
    writeln!(out, "      }}").unwrap();
    writeln!(
        out,
        "      const error = new {error_name}(message, response.status, errorBody);"
    )
    .unwrap();
    writeln!(out, "      if (options.onError) options.onError(error);").unwrap();
    writeln!(out, "      throw error;").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    // Parse response
    writeln!(out, "    const text = await response.text();").unwrap();
    writeln!(
        out,
        "    return (text ? JSON.parse(text) : undefined) as T;"
    )
    .unwrap();
    writeln!(out, "  }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "  return request;").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
}

/// Generate routes organized into groups (nested objects).
fn generate_grouped_routes(out: &mut String, routes: &RouteCollection) {
    // Separate ungrouped and grouped routes
    let mut ungrouped: Vec<&RouteDefinition> = Vec::new();
    let mut groups: BTreeMap<String, Vec<&RouteDefinition>> = BTreeMap::new();

    for route in routes {
        match &route.group {
            Some(group) => groups.entry(group.clone()).or_default().push(route),
            None => ungrouped.push(route),
        }
    }

    // Ungrouped routes first (indent = 4 spaces)
    for route in &ungrouped {
        write!(out, "    ").unwrap();
        generate_route_method(out, route, 4);
        writeln!(out, ",").unwrap();
    }

    // Grouped routes (indent = 6 spaces)
    for (group_name, group_routes) in &groups {
        if !ungrouped.is_empty() || groups.keys().next() != Some(group_name) {
            writeln!(out).unwrap();
        }
        writeln!(out, "    {group_name}: {{").unwrap();
        for route in group_routes {
            write!(out, "      ").unwrap();
            generate_route_method(out, route, 6);
            writeln!(out, ",").unwrap();
        }
        writeln!(out, "    }},").unwrap();
    }
}

/// Generate routes in a flat structure (no grouping).
fn generate_flat_routes(out: &mut String, routes: &RouteCollection) {
    for route in routes {
        write!(out, "    ").unwrap();
        generate_route_method(out, route, 4);
        writeln!(out, ",").unwrap();
    }
}

/// Generate a single route method.
///
/// `indent` is the current indentation level in spaces (for continuation lines).
fn generate_route_method(out: &mut String, route: &RouteDefinition, indent: usize) {
    let name = &route.name;
    let params = generate_params(route);
    let path_template = build_path_template(&route.path);
    let cont_indent = " ".repeat(indent + 2); // continuation indentation

    if route.redirect {
        // Redirect routes generate URL-builder functions
        generate_redirect_method(out, route, name, &params, &path_template, indent);
    } else {
        // Normal fetch routes
        let return_type = route
            .response_type
            .as_ref()
            .map(|t| rust_type_to_ts(t))
            .unwrap_or_else(|| "void".into());

        let opts = generate_request_options(route);

        if params.is_empty() {
            write!(
                out,
                "{name}: () => request<{return_type}>({path_template}{opts})"
            )
            .unwrap();
        } else {
            write!(
                out,
                "{name}: ({params}) =>\n{cont_indent}request<{return_type}>({path_template}{opts})"
            )
            .unwrap();
        }
    }
}

/// Generate a redirect route (URL-builder, not fetch).
fn generate_redirect_method(
    out: &mut String,
    route: &RouteDefinition,
    name: &str,
    params: &str,
    path_template: &str,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let pad2 = " ".repeat(indent + 2);
    let pad4 = " ".repeat(indent + 4);

    // For redirect routes with query params, build URL with optional query string
    if route.query_type.is_some() {
        let query_ts = route
            .query_type
            .as_ref()
            .map(|t| rust_type_to_ts(t))
            .unwrap_or_default();

        // Rebuild params without query (which was already added by generate_params)
        let mut fn_params = Vec::new();
        for param in &route.path_params {
            fn_params.push(format!("{}: string", param.name));
        }
        fn_params.push(format!("query?: {query_ts}"));

        let all_params = fn_params.join(", ");

        writeln!(out, "{name}: ({all_params}) => {{").unwrap();
        writeln!(
            out,
            "{pad2}let url = `${{options.baseUrl}}{}`;\n{pad2}if (query) {{",
            &path_template[1..path_template.len() - 1] // strip outer quotes/backticks
        )
        .unwrap();
        writeln!(out, "{pad4}const params = new URLSearchParams();").unwrap();
        writeln!(
            out,
            "{pad4}for (const [key, value] of Object.entries(query)) {{"
        )
        .unwrap();
        writeln!(
            out,
            "{pad4}  if (value !== undefined && value !== null) params.set(key, String(value));"
        )
        .unwrap();
        writeln!(out, "{pad4}}}").unwrap();
        writeln!(out, "{pad4}const qs = params.toString();").unwrap();
        writeln!(out, "{pad4}if (qs) url += `?${{qs}}`;").unwrap();
        writeln!(out, "{pad2}}}").unwrap();
        write!(out, "{pad2}return url;\n{pad}}}").unwrap();
    } else {
        // Simple URL builder with just path params
        if params.is_empty() {
            write!(
                out,
                "{name}: () => `${{options.baseUrl}}{}`",
                &path_template[1..path_template.len() - 1]
            )
            .unwrap();
        } else {
            write!(
                out,
                "{name}: ({params}) => `${{options.baseUrl}}{}`",
                &path_template[1..path_template.len() - 1]
            )
            .unwrap();
        }
    }
}

/// Derive a type name from a factory function name.
///
/// `createYAuthClient` -> `YAuthClient`
/// `createApiClient` -> `ApiClient`
fn derive_type_name(factory_name: &str) -> String {
    factory_name
        .strip_prefix("create")
        .unwrap_or(factory_name)
        .to_string()
}

/// Generate the client and write it to a file.
pub fn generate_to_file(
    routes: &RouteCollection,
    config: &GeneratorConfig,
) -> Result<(), std::io::Error> {
    let content = generate(routes, config);

    // Ensure parent directory exists
    if let Some(parent) = Path::new(&config.output_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&config.output_path, &content)
}

/// Check if the generated output matches the committed file.
///
/// Returns `Ok(())` if in sync, `Err(CheckError)` if not.
pub fn check(routes: &RouteCollection, config: &GeneratorConfig) -> Result<(), CheckError> {
    let generated = generate(routes, config);

    let existing =
        std::fs::read_to_string(&config.output_path).map_err(|e| CheckError::ReadError {
            path: config.output_path.clone(),
            error: e,
        })?;

    if generated != existing {
        Err(CheckError::OutOfSync {
            path: config.output_path.clone(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_type_to_ts_primitives() {
        assert_eq!(rust_type_to_ts("String"), "string");
        assert_eq!(rust_type_to_ts("bool"), "boolean");
        assert_eq!(rust_type_to_ts("u32"), "number");
        assert_eq!(rust_type_to_ts("i64"), "number");
        assert_eq!(rust_type_to_ts("Uuid"), "string");
        assert_eq!(rust_type_to_ts("f64"), "number");
    }

    #[test]
    fn test_rust_type_to_ts_vec() {
        assert_eq!(rust_type_to_ts("Vec<String>"), "string[]");
        assert_eq!(rust_type_to_ts("Vec<UserResponse>"), "UserResponse[]");
    }

    #[test]
    fn test_rust_type_to_ts_option() {
        assert_eq!(rust_type_to_ts("Option<String>"), "string | null");
        assert_eq!(
            rust_type_to_ts("Option<UserResponse>"),
            "UserResponse | null"
        );
    }

    #[test]
    fn test_rust_type_to_ts_custom() {
        assert_eq!(rust_type_to_ts("RegisterRequest"), "RegisterRequest");
        assert_eq!(rust_type_to_ts("LoginResponse"), "LoginResponse");
    }

    #[test]
    fn test_build_path_template_no_params() {
        assert_eq!(build_path_template("/register"), "\"/register\"");
        assert_eq!(build_path_template("/admin/users"), "\"/admin/users\"");
    }

    #[test]
    fn test_build_path_template_with_params() {
        assert_eq!(
            build_path_template("/admin/users/{id}"),
            "`/admin/users/${id}`"
        );
        assert_eq!(
            build_path_template("/orgs/{org}/users/{id}"),
            "`/orgs/${org}/users/${id}`"
        );
    }

    #[test]
    fn test_derive_type_name() {
        assert_eq!(derive_type_name("createYAuthClient"), "YAuthClient");
        assert_eq!(derive_type_name("createApiClient"), "ApiClient");
        assert_eq!(derive_type_name("myClient"), "myClient");
    }

    #[test]
    fn test_is_primitive_type() {
        assert!(is_primitive_type("String"));
        assert!(is_primitive_type("bool"));
        assert!(is_primitive_type("u32"));
        assert!(is_primitive_type("Uuid"));
        assert!(!is_primitive_type("RegisterRequest"));
        assert!(is_primitive_type("Vec<String>"));
        assert!(!is_primitive_type("Vec<UserResponse>"));
    }

    #[test]
    fn test_base_type_name() {
        assert_eq!(base_type_name("String"), None);
        assert_eq!(
            base_type_name("RegisterRequest"),
            Some("RegisterRequest".into())
        );
        assert_eq!(
            base_type_name("Vec<UserResponse>"),
            Some("UserResponse".into())
        );
        assert_eq!(base_type_name("Vec<String>"), None);
        assert_eq!(
            base_type_name("Option<UserResponse>"),
            Some("UserResponse".into())
        );
    }

    #[test]
    fn test_compute_import_prefix_explicit() {
        let config = GeneratorConfig {
            type_import_prefix: "../types".into(),
            ..Default::default()
        };
        assert_eq!(compute_import_prefix(&config), "../types");
    }
}
