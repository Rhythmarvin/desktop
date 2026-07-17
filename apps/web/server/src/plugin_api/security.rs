use axum::extract::{Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, AUTHORIZATION, HOST, ORIGIN,
    VARY,
};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::BTreeSet;

/// Immutable per-process HTTP authority for the loopback plugin adapter.
#[derive(Clone)]
pub struct PluginSecurity {
    bearer: [u8; 32],
    expected_host: String,
    allowed_origins: BTreeSet<String>,
}

impl PluginSecurity {
    pub fn new(
        bearer: [u8; 32],
        expected_host: String,
        allowed_origins: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        let allowed_origins = allowed_origins.into_iter().collect::<BTreeSet<_>>();
        if expected_host.is_empty()
            || allowed_origins.is_empty()
            || allowed_origins
                .iter()
                .any(|origin| origin.is_empty() || origin == "null" || origin == "*")
        {
            return Err("plugin HTTP security policy is invalid".to_owned());
        }
        Ok(Self {
            bearer,
            expected_host,
            allowed_origins,
        })
    }

    /// Encodes the in-memory bearer for the trusted Tauri bootstrap channel only.
    pub fn bearer_hex(&self) -> String {
        self.bearer
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// Enforces loopback Host, exact Origin, preflight policy, and constant-time bearer comparison.
pub async fn enforce(
    State(security): State<PluginSecurity>,
    request: Request,
    next: Next,
) -> Response {
    let origin = match exact_header(&request, ORIGIN) {
        Some(origin) if security.allowed_origins.contains(origin) => origin.to_owned(),
        _ => return StatusCode::FORBIDDEN.into_response(),
    };
    if exact_header(&request, HOST) != Some(security.expected_host.as_str()) {
        return cors_error(StatusCode::FORBIDDEN, &origin);
    }
    if request.method() == Method::OPTIONS {
        return preflight(&security, &request, &origin);
    }
    if !authorized(&security, &request) {
        return cors_error(StatusCode::UNAUTHORIZED, &origin);
    }
    let mut response = next.run(request).await;
    add_cors_headers(response.headers_mut(), &origin);
    response
}

fn preflight(security: &PluginSecurity, request: &Request, origin: &str) -> Response {
    let Some(requested_method) = exact_header(request, ACCESS_CONTROL_REQUEST_METHOD) else {
        return cors_error(StatusCode::BAD_REQUEST, origin);
    };
    if !preflight_method_allowed(request.uri().path(), requested_method) {
        return cors_error(StatusCode::METHOD_NOT_ALLOWED, origin);
    }
    let requested_headers = exact_header(request, ACCESS_CONTROL_REQUEST_HEADERS).unwrap_or("");
    if requested_headers.split(',').any(|header| {
        let header = header.trim();
        !header.is_empty()
            && !header.eq_ignore_ascii_case("authorization")
            && !header.eq_ignore_ascii_case("content-type")
    }) {
        return cors_error(StatusCode::FORBIDDEN, origin);
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    add_cors_headers(response.headers_mut(), origin);
    response.headers_mut().insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_str(requested_method).unwrap_or(HeaderValue::from_static("POST")),
    );
    response.headers_mut().insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type"),
    );
    let _ = security;
    response
}

fn preflight_method_allowed(path: &str, method: &str) -> bool {
    match path {
        "/api/plugins" => method == "GET",
        "/api/plugins/scan" | "/api/plugins/identify" | "/api/plugins/install" => method == "POST",
        "/api/agent-invocations" => method == "POST",
        _ if path.starts_with("/api/agent-invocations/") => method == "DELETE",
        _ if path.starts_with("/api/plugins/") && path.ends_with("/launch-grant") => {
            matches!(method, "GET" | "PUT" | "DELETE")
        }
        _ if path.starts_with("/api/plugins/")
            && (path.ends_with("/enable")
                || path.ends_with("/disable")
                || path.ends_with("/reset-crash-loop")
                || path.ends_with("/remove-data")
                || path.ends_with("/start")
                || path.ends_with("/stop")) =>
        {
            method == "POST"
        }
        _ if path.starts_with("/api/plugins/") => method == "DELETE",
        _ => false,
    }
}

fn authorized(security: &PluginSecurity, request: &Request) -> bool {
    let Some(header) = exact_header(request, AUTHORIZATION) else {
        return false;
    };
    let Some(encoded) = header.strip_prefix("Bearer ") else {
        return false;
    };
    let mut supplied = [0u8; 32];
    let mut invalid = usize::from(encoded.len() != 64);
    for (index, slot) in supplied.iter_mut().enumerate() {
        let offset = index * 2;
        let high = encoded.as_bytes().get(offset).copied().and_then(hex_value);
        let low = encoded
            .as_bytes()
            .get(offset + 1)
            .copied()
            .and_then(hex_value);
        match (high, low) {
            (Some(high), Some(low)) => *slot = (high << 4) | low,
            _ => invalid |= 1,
        }
    }
    let difference = supplied
        .iter()
        .zip(security.bearer)
        .fold(invalid, |difference, (left, right)| {
            difference | usize::from(*left ^ right)
        });
    difference == 0
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn exact_header(request: &Request, name: axum::http::HeaderName) -> Option<&str> {
    let mut values = request.headers().get_all(name).iter();
    let first = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(first)
}

fn cors_error(status: StatusCode, origin: &str) -> Response {
    let mut response = status.into_response();
    add_cors_headers(response.headers_mut(), origin);
    response
}

fn add_cors_headers(headers: &mut axum::http::HeaderMap, origin: &str) {
    if let Ok(origin) = HeaderValue::from_str(origin) {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    headers.insert(VARY, HeaderValue::from_static("Origin"));
}

#[cfg(test)]
mod tests {
    use super::{PluginSecurity, authorized, enforce};
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header::AUTHORIZATION};
    use axum::middleware;
    use axum::response::Response;
    use axum::routing::{get, post};
    use bytes::Bytes;
    use futures_util::stream;
    use pretty_assertions::assert_eq;
    use std::convert::Infallible;
    use tower::ServiceExt;

    #[test]
    fn compares_only_exact_lowercase_bearer_encoding() {
        let security = PluginSecurity::new(
            [0xabu8; 32],
            "127.0.0.1:42".to_owned(),
            ["tauri://localhost".to_owned()],
        )
        .unwrap_or_else(|error| panic!("security: {error}"));
        let valid = Request::builder()
            .header(AUTHORIZATION, format!("Bearer {}", "ab".repeat(32)))
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request: {error}"));
        let invalid = Request::builder()
            .header(AUTHORIZATION, format!("Bearer {}", "AB".repeat(32)))
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request: {error}"));
        assert!(authorized(&security, &valid));
        assert!(!authorized(&security, &invalid));
    }

    /// Requires exact Host/Origin/auth while permitting a constrained bearer-free preflight.
    #[tokio::test]
    async fn enforces_actual_and_preflight_requests() {
        let security = PluginSecurity::new(
            [0xabu8; 32],
            "127.0.0.1:42".to_owned(),
            ["tauri://localhost".to_owned()],
        )
        .unwrap_or_else(|error| panic!("security: {error}"));
        let app = Router::new()
            .route(
                "/api/plugins",
                get(|| async { StatusCode::OK }).options(|| async { StatusCode::NO_CONTENT }),
            )
            .layer(middleware::from_fn_with_state(security, enforce));
        let authorized = app
            .clone()
            .oneshot(request(
                Method::GET,
                Some(&format!("Bearer {}", "ab".repeat(32))),
            ))
            .await
            .unwrap_or_else(|error| panic!("authorized request: {error}"));
        assert_eq!(authorized.status(), StatusCode::OK);
        assert_eq!(
            authorized.headers()["access-control-allow-origin"],
            "tauri://localhost"
        );

        let unauthorized = app
            .clone()
            .oneshot(request(Method::GET, None))
            .await
            .unwrap_or_else(|error| panic!("unauthorized request: {error}"));
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let preflight = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/api/plugins")
                    .header("host", "127.0.0.1:42")
                    .header("origin", "tauri://localhost")
                    .header("access-control-request-method", "GET")
                    .header("access-control-request-headers", "Authorization")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("preflight request: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("preflight response: {error}"));
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
    }

    /// Rejects every Host/Origin/preflight policy deviation with stable CORS visibility.
    #[tokio::test]
    async fn rejects_host_origin_method_and_header_deviations() {
        let app = secured_router();
        let bearer = format!("Bearer {}", "ab".repeat(32));
        let wrong_host = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/plugins")
                    .header("host", "localhost:42")
                    .header("origin", "tauri://localhost")
                    .header(AUTHORIZATION, &bearer)
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("wrong-host request: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("wrong-host response: {error}"));
        assert_eq!(
            (
                wrong_host.status(),
                wrong_host
                    .headers()
                    .get("access-control-allow-origin")
                    .and_then(|value| value.to_str().ok()),
                wrong_host
                    .headers()
                    .get("vary")
                    .and_then(|value| value.to_str().ok()),
            ),
            (
                StatusCode::FORBIDDEN,
                Some("tauri://localhost"),
                Some("Origin"),
            )
        );

        let wrong_origin = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/plugins")
                    .header("host", "127.0.0.1:42")
                    .header("origin", "https://attacker.invalid")
                    .header(AUTHORIZATION, &bearer)
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("wrong-origin request: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("wrong-origin response: {error}"));
        assert_eq!(
            (
                wrong_origin.status(),
                wrong_origin
                    .headers()
                    .contains_key("access-control-allow-origin"),
            ),
            (StatusCode::FORBIDDEN, false)
        );

        let wrong_method = app
            .clone()
            .oneshot(preflight_request("PATCH", "Authorization"))
            .await
            .unwrap_or_else(|error| panic!("wrong-method response: {error}"));
        assert_eq!(wrong_method.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            wrong_method.headers()["access-control-allow-origin"],
            "tauri://localhost"
        );

        let wrong_header = app
            .oneshot(preflight_request("GET", "Authorization, X-Injected"))
            .await
            .unwrap_or_else(|error| panic!("wrong-header response: {error}"));
        assert_eq!(wrong_header.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            wrong_header.headers()["access-control-allow-origin"],
            "tauri://localhost"
        );
    }

    /// Adds identical exact-Origin headers to application errors and streamed bodies.
    #[tokio::test]
    async fn decorates_error_and_stream_responses() {
        let security = test_security();
        let app = Router::new()
            .route(
                "/api/plugins",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .route(
                "/api/agent-invocations",
                post(|| async {
                    Response::new(Body::from_stream(stream::iter([Ok::<_, Infallible>(
                        Bytes::from_static(b"{\"type\":\"terminal\"}\n"),
                    )])))
                }),
            )
            .layer(middleware::from_fn_with_state(security, enforce));
        let bearer = format!("Bearer {}", "ab".repeat(32));
        let error = app
            .clone()
            .oneshot(request(Method::GET, Some(&bearer)))
            .await
            .unwrap_or_else(|error| panic!("application error response: {error}"));
        assert_eq!(
            (
                error.status(),
                error.headers()["access-control-allow-origin"].clone(),
                error.headers()["vary"].clone(),
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "tauri://localhost".parse().unwrap_or_else(|parse_error| {
                    panic!("expected origin header: {parse_error}")
                }),
                "Origin"
                    .parse()
                    .unwrap_or_else(|parse_error| panic!("expected vary header: {parse_error}")),
            )
        );

        let streamed = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/agent-invocations")
                    .header("host", "127.0.0.1:42")
                    .header("origin", "tauri://localhost")
                    .header(AUTHORIZATION, bearer)
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("stream request: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("stream response: {error}"));
        assert_eq!(
            (
                streamed.status(),
                streamed.headers()["access-control-allow-origin"].clone(),
                streamed.headers()["vary"].clone(),
            ),
            (
                StatusCode::OK,
                "tauri://localhost".parse().unwrap_or_else(|parse_error| {
                    panic!("expected origin header: {parse_error}")
                }),
                "Origin"
                    .parse()
                    .unwrap_or_else(|parse_error| panic!("expected vary header: {parse_error}")),
            )
        );
    }

    /// Builds the shared security policy used by HTTP boundary tests.
    fn test_security() -> PluginSecurity {
        PluginSecurity::new(
            [0xabu8; 32],
            "127.0.0.1:42".to_owned(),
            ["tauri://localhost".to_owned()],
        )
        .unwrap_or_else(|error| panic!("security: {error}"))
    }

    /// Builds a secured catalog router for negative Host/Origin/preflight requests.
    fn secured_router() -> Router {
        Router::new()
            .route(
                "/api/plugins",
                get(|| async { StatusCode::OK }).options(|| async { StatusCode::NO_CONTENT }),
            )
            .layer(middleware::from_fn_with_state(test_security(), enforce))
    }

    /// Builds a bearer-free preflight that still carries every required security header.
    fn preflight_request(method: &str, headers: &str) -> Request<Body> {
        Request::builder()
            .method(Method::OPTIONS)
            .uri("/api/plugins")
            .header("host", "127.0.0.1:42")
            .header("origin", "tauri://localhost")
            .header("access-control-request-method", method)
            .header("access-control-request-headers", headers)
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("preflight request: {error}"))
    }

    fn request(method: Method, authorization: Option<&str>) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri("/api/plugins")
            .header("host", "127.0.0.1:42")
            .header("origin", "tauri://localhost");
        if let Some(authorization) = authorization {
            request = request.header(AUTHORIZATION, authorization);
        }
        request
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request: {error}"))
    }
}
