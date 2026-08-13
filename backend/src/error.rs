//! Standardized API error model.
//!
//! Every error converts to the same JSON shape:
//! `{"error": {"code": ..., "message": ..., "request_id": ...}}`.
//! Stack traces are never leaked to clients.

use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
// Used by auth/rate-limit middleware (Phases 3-4); not yet reachable.
#[allow(dead_code)]
pub enum ApiErrorKind {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("rate limit exceeded")]
    RateLimitExceeded { retry_after_seconds: u64 },
    #[error("usage limit exceeded")]
    QuotaExceeded,
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug)]
// Used by auth/rate-limit middleware (Phases 3-4); not yet reachable.
#[allow(dead_code)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind) -> Self {
        Self {
            kind,
            request_id: None,
        }
    }

    // Used once handlers thread the request id into errors (Phase 3+).
    #[allow(dead_code)]
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    fn code(&self) -> &'static str {
        match &self.kind {
            ApiErrorKind::Unauthorized => "unauthorized",
            ApiErrorKind::Forbidden => "forbidden",
            ApiErrorKind::RateLimitExceeded { .. } => "rate_limit_exceeded",
            ApiErrorKind::QuotaExceeded => "quota_exceeded",
            ApiErrorKind::ServiceUnavailable(_) => "service_unavailable",
            ApiErrorKind::InvalidRequest(_) => "invalid_request",
            ApiErrorKind::NotFound => "not_found",
            ApiErrorKind::Conflict(_) => "conflict",
            ApiErrorKind::Internal(_) => "internal_error",
        }
    }

    fn status(&self) -> StatusCode {
        match &self.kind {
            ApiErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ApiErrorKind::RateLimitExceeded { .. } | ApiErrorKind::QuotaExceeded => {
                StatusCode::TOO_MANY_REQUESTS
            }
            ApiErrorKind::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ApiErrorKind::NotFound => StatusCode::NOT_FOUND,
            ApiErrorKind::Conflict(_) => StatusCode::CONFLICT,
            ApiErrorKind::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorKind::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let code = self.code();
        let status = self.status();
        let message = self.kind.to_string();
        let retry_after = match &self.kind {
            ApiErrorKind::RateLimitExceeded {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            _ => None,
        };
        let request_id = self.request_id;

        let body = json!({
            "error": {
                "code": code,
                "message": message,
                "request_id": request_id,
            }
        });

        let mut response = (status, Json(body)).into_response();
        if let Some(retry) = retry_after {
            if let Ok(v) = HeaderValue::from_str(&retry.to_string()) {
                response.headers_mut().insert("retry-after", v);
            }
        }
        response
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(error = %err, "database error");
        ApiError::new(ApiErrorKind::Internal(anyhow::Error::new(err)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(response: Response) -> serde_json::Value {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024)
                .await
                .expect("read body");
            serde_json::from_slice(&bytes).expect("json body")
        })
    }

    #[test]
    fn error_shape_is_standardized() {
        let response = ApiError::new(ApiErrorKind::Unauthorized).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let value = body(response);
        assert_eq!(value["error"]["code"], "unauthorized");
        assert_eq!(value["error"]["message"], "unauthorized");
        assert!(value["error"]["request_id"].is_null());
    }

    #[test]
    fn rate_limit_sets_retry_after_and_429() {
        let response = ApiError::new(ApiErrorKind::RateLimitExceeded {
            retry_after_seconds: 12,
        })
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get("retry-after").unwrap(),
            HeaderValue::from_static("12")
        );
        let value = body(response);
        assert_eq!(value["error"]["code"], "rate_limit_exceeded");
    }

    #[test]
    fn carries_request_id_when_provided() {
        let response = ApiError::new(ApiErrorKind::NotFound)
            .with_request_id(Some("abc-123".to_string()))
            .into_response();
        let value = body(response);
        assert_eq!(value["error"]["request_id"], "abc-123");
    }
}
