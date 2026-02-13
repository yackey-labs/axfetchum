/// Declarative macro for defining API route metadata.
///
/// # Syntax
///
/// ```rust,ignore
/// use axum_ts_client::api_routes;
///
/// let routes = api_routes! {
///     @group emailPassword
///
///     register: POST "/register"
///         body: RegisterRequest -> MessageResponse;
///     login: POST "/login"
///         body: LoginRequest -> LoginResponse;
///     verify: POST "/verify-email"
///         body: VerifyEmailRequest -> MessageResponse;
///     changePassword: POST "/change-password" [auth]
///         body: ChangePasswordRequest -> MessageResponse;
///     listUsers: GET "/admin/users" [auth]
///         query: ListUsersQuery -> ListUsersResponse;
///     getUser: GET "/admin/users/{id}" [auth]
///         -> UserResponse;
///     authorize: GET "/oauth/{provider}/authorize" [redirect]
///         query: AuthorizeQuery;
/// };
/// ```
///
/// # Elements
///
/// - `@group <name>` — sets the group for all following routes (generates nested object)
/// - `[auth]` — marks route as requiring authentication
/// - `[redirect]` — marks route as a browser redirect (URL builder, not fetch)
/// - `body: <Type>` — request body type
/// - `query: <Type>` — query parameters type
/// - `-> <Type>` — response type (omit for void)
/// - `{param}` in paths — path parameters (become function args)
#[macro_export]
macro_rules! api_routes {
    // Entry point — parse all statements
    (@collect $collection:ident, @group_ctx $group:expr, ) => {};

    // @group directive — set group context for subsequent routes
    (@collect $collection:ident, @group_ctx $_old_group:expr,
        @group $new_group:ident
        $($rest:tt)*
    ) => {
        $crate::api_routes!(@collect $collection, @group_ctx Some(stringify!($new_group).to_string()), $($rest)*);
    };

    // @nogroup directive — clear group context
    (@collect $collection:ident, @group_ctx $_old_group:expr,
        @nogroup
        $($rest:tt)*
    ) => {
        $crate::api_routes!(@collect $collection, @group_ctx Option::<String>::None, $($rest)*);
    };

    // Route: method path [flags] body: Type query: Type -> ResponseType;
    // We parse each route by matching the name, method, path, then optional parts
    (@collect $collection:ident, @group_ctx $group:expr,
        $name:ident : $method:ident $path:literal
        $([$($flag:ident),*])?
        $(body: $body_ty:ident)?
        $(query: $query_ty:ident)?
        $(-> $resp_ty:ident)?
        ;
        $($rest:tt)*
    ) => {
        $collection.push($crate::RouteDefinition {
            name: stringify!($name).to_string(),
            method: $crate::api_routes!(@method $method),
            path: $path.to_string(),
            auth: $crate::api_routes!(@has_flag auth $([$($flag),*])?),
            body_type: $crate::api_routes!(@opt_type $($body_ty)?),
            response_type: $crate::api_routes!(@opt_type $($resp_ty)?),
            query_type: $crate::api_routes!(@opt_type $($query_ty)?),
            path_params: $crate::extract_path_params($path),
            group: $group.clone(),
            redirect: $crate::api_routes!(@has_flag redirect $([$($flag),*])?),
        });
        $crate::api_routes!(@collect $collection, @group_ctx $group, $($rest)*);
    };

    // Method helpers
    (@method GET) => { $crate::HttpMethod::Get };
    (@method POST) => { $crate::HttpMethod::Post };
    (@method PUT) => { $crate::HttpMethod::Put };
    (@method PATCH) => { $crate::HttpMethod::Patch };
    (@method DELETE) => { $crate::HttpMethod::Delete };

    // Flag detection helpers
    (@has_flag $target:ident) => { false };
    (@has_flag $target:ident [$($flag:ident),*]) => {
        $crate::api_routes!(@check_flag $target, $($flag),*)
    };
    (@check_flag $target:ident, ) => { false };
    (@check_flag $target:ident, $target2:ident $(, $rest:ident)*) => {
        $crate::api_routes!(@flag_eq $target $target2) || $crate::api_routes!(@check_flag $target, $($rest),*)
    };

    // Compare two idents for equality via stringify
    (@flag_eq $a:ident $b:ident) => {
        {
            // This is a const-evaluable string comparison
            const A: &str = stringify!($a);
            const B: &str = stringify!($b);
            A.len() == B.len() && {
                let a = A.as_bytes();
                let b = B.as_bytes();
                let mut i = 0;
                let mut eq = true;
                while i < a.len() {
                    if a[i] != b[i] {
                        eq = false;
                    }
                    i += 1;
                }
                eq
            }
        }
    };

    // Optional type helpers
    (@opt_type) => { None };
    (@opt_type $ty:ident) => { Some(stringify!($ty).to_string()) };

    // Top-level entry point
    {
        $($tokens:tt)*
    } => {
        {
            let mut collection = $crate::RouteCollection::new();
            $crate::api_routes!(@collect collection, @group_ctx Option::<String>::None, $($tokens)*);
            collection
        }
    };
}
