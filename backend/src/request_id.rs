//! Request ID middleware.
//!
//! Every request gets a `request_id` (UUID). An incoming `X-Request-ID` is
//! accepted only when it parses as a UUID; otherwise a fresh one is generated.
//! The id is attached to the request extensions, echoed back in the
//! `X-Request-ID` response header, and is available to handlers, logs, usage
//! records, analytics events, and error responses.

use axum::body::Body;
use axum::http::Request;
use axum::http::header::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

/// Extension type holding the current request id.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub async fn layer(mut request: Request<Body>, next: Next) -> Response {
    let id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| uuid::Uuid::parse_str(v).is_ok())
        .map(|v| v.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    request.extensions_mut().insert(RequestId(id.clone()));

    let mut response = next.run(request).await;
    if let Ok(v) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", v);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, StatusCode};
    use axum::middleware;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use tower::ServiceExt;

    async fn echo_handler(request: axum::extract::Request) -> Response {
        let id = request
            .extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_default();
        (StatusCode::OK, id).into_response()
    }

    struct RunResult {
        header: String,
        body: String,
        status: StatusCode,
    }

    fn run(request: axum::http::Request<Body>) -> RunResult {
        let app = Router::new()
            .route("/", get(echo_handler))
            .layer(middleware::from_fn(layer));
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let header = response
                .headers()
                .get("x-request-id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let body = String::from_utf8(
                to_bytes(response.into_body(), 16 * 1024)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            RunResult {
                header,
                body,
                status,
            }
        })
    }

    #[test]
    fn generates_and_stamps_request_id() {
        let request = axum::http::Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let result = run(request);
        assert_eq!(result.status, StatusCode::OK);
        assert!(uuid::Uuid::parse_str(&result.header).is_ok());
        assert_eq!(result.body, result.header);
    }

    #[test]
    fn accepts_valid_incoming_uuid() {
        let id = "123e4567-e89b-12d3-a456-426614174000";
        let request = axum::http::Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("x-request-id", id)
            .body(Body::empty())
            .unwrap();
        let result = run(request);
        assert_eq!(result.header, id);
    }

    #[test]
    fn rejects_invalid_incoming_uuid() {
        let request = axum::http::Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("x-request-id", "not-a-uuid")
            .body(Body::empty())
            .unwrap();
        let result = run(request);
        assert!(uuid::Uuid::parse_str(&result.header).is_ok());
        assert_ne!(result.header, "not-a-uuid");
    }
}
