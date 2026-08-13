//! Authentication middleware: resolves the session cookie into a
//! [`UserContext`] carrying the user, role, and permissions.

use std::collections::HashSet;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::sessions;
use super::users;
use crate::error::{ApiError, ApiErrorKind};
use crate::request_id::RequestId;
use crate::state::AppState;

/// Authenticated request context attached to protected handlers.
#[derive(Debug, Clone)]
// Some fields are consumed by rate limiting (role_id) and admin analytics
// (email/name) in later phases.
#[allow(dead_code)]
pub struct UserContext {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub role_id: Uuid,
    pub role_name: String,
    pub email: String,
    pub name: Option<String>,
    pub permissions: HashSet<String>,
    /// Current request id, threaded into error responses and audit logs.
    pub request_id: String,
    /// Hashed client IP for audit logging.
    pub ip_hash: String,
}

impl UserContext {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }
}

/// Rejects a handler with `403 Forbidden` when the user lacks `permission`.
/// Use this instead of sprinkling role-name checks through handlers.
pub fn require_permission(ctx: &UserContext, permission: &str) -> Result<(), ApiError> {
    if ctx.has_permission(permission) {
        Ok(())
    } else {
        Err(ApiError::new(ApiErrorKind::Forbidden).with_request_id(Some(ctx.request_id.clone())))
    }
}

fn client_ip(parts: &Parts) -> String {
    parts
        .headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}

/// Fast extractor: reads the [`UserContext`] that a middleware already resolved
/// and attached to request extensions. Only valid inside routes wrapped by the
/// graph rate-limit middleware.
// The inner context is consumed by usage metering (Phase 5).
#[allow(dead_code)]
pub struct AuthUser(pub UserContext);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let request_id = parts
            .extensions
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "-".into());
        match parts.extensions.get::<UserContext>().cloned() {
            Some(ctx) => Ok(AuthUser(ctx)),
            None => {
                Err(ApiError::new(ApiErrorKind::Unauthorized).with_request_id(Some(request_id)))
            }
        }
    }
}

impl FromRequestParts<AppState> for UserContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let request_id = parts
            .extensions
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "-".into());

        let unauthorized =
            || ApiError::new(ApiErrorKind::Unauthorized).with_request_id(Some(request_id.clone()));

        let jar = CookieJar::from_headers(&parts.headers);
        let Some(token) = jar
            .get(&state.config.session_cookie_name)
            .map(|c| c.value().to_string())
        else {
            return Err(unauthorized());
        };

        let su = sessions::lookup_session(&state.db, &token)
            .await
            .map_err(|e| ApiError::new(ApiErrorKind::Internal(anyhow::Error::new(e))))?;
        let Some(su) = su else {
            return Err(unauthorized());
        };

        if su.revoked_at.is_some() || su.expires_at < Utc::now() || su.status != "active" {
            return Err(unauthorized());
        }

        let permissions = users::load_permissions(&state.db, su.role_id)
            .await
            .map_err(|e| ApiError::new(ApiErrorKind::Internal(anyhow::Error::new(e))))?;

        // Best-effort heartbeat; never fail the request on a write hiccup.
        sessions::touch_session(&state.db, su.session_id).await;

        let ip_hash = hex::encode(Sha256::digest(client_ip(parts).as_bytes()));

        Ok(UserContext {
            user_id: su.user_id,
            session_id: su.session_id,
            role_id: su.role_id,
            role_name: su.role_name,
            email: su.email,
            name: su.name,
            permissions,
            request_id,
            ip_hash,
        })
    }
}
