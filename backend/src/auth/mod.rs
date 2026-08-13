//! Authentication routes and handlers: Google OAuth flow, sessions, logout,
//! `/auth/me`, and the dev-only test login stub.

pub mod middleware;
pub mod oauth;
pub mod sessions;
pub mod users;

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::header::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use time::Duration as CookieDuration;

use crate::error::{ApiError, ApiErrorKind};
use crate::state::AppState;
use oauth::OidcIdentity;

const OAUTH_STATE_PREFIX: &str = "oauth:state:";
const AUTH_INIT_KEY_PREFIX: &str = "auth:init:";
const AUTH_CALLBACK_KEY_PREFIX: &str = "auth:cb:";
const STATE_TTL_SECONDS: u64 = 600;

/// Build the `/auth/*` router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/google", get(google_login))
        .route("/auth/google/callback", get(google_callback))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
        .route("/auth/me", get(me))
        .route("/auth/test/login", post(test_login))
}

// ---------------------------------------------------------------------------
// Cookie helpers
// ---------------------------------------------------------------------------

fn build_session_cookie(config: &crate::config::Config, token: &str) -> Cookie<'static> {
    let secure = config.frontend_url.starts_with("https://");
    Cookie::build((config.session_cookie_name.clone(), token.to_string()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .path("/")
        .max_age(CookieDuration::seconds(config.session_ttl_seconds))
        .build()
}

fn remove_session_cookie(config: &crate::config::Config) -> Cookie<'static> {
    Cookie::from(config.session_cookie_name.clone())
}

fn redirect_frontend(config: &crate::config::Config, path: &str) -> Response {
    Redirect::to(&format!("{}{}", config.frontend_url, path)).into_response()
}

fn hash_ip(ip: &str) -> String {
    hex::encode(Sha256::digest(ip.as_bytes()))
}

/// Client IP honoring `X-Forwarded-For` when present (reverse-proxy topology).
fn request_ip(headers: &HeaderMap, addr: Option<SocketAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            addr.map(|a| a.ip().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        })
}

// ---------------------------------------------------------------------------
// Google login (browser navigation)
// ---------------------------------------------------------------------------

async fn google_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Redirect, ApiError> {
    if state.config.google_client_id.is_none() {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "Google OAuth is not configured".into(),
        )));
    }

    // Login-abuse throttle: IP-based, short-lived. Fail-open on Redis errors so
    // an outage cannot lock out all users; the counter is best-effort only.
    let ip = request_ip(&headers, Some(addr));
    let key = format!("{AUTH_INIT_KEY_PREFIX}{ip}");
    match state.redis.increment_window(&key, STATE_TTL_SECONDS).await {
        Ok(count) if count > state.config.oauth_init_limit_per_10_min => {
            tracing::warn!(ip = %ip, count, "oauth init throttled");
            return Err(ApiError::new(ApiErrorKind::RateLimitExceeded {
                retry_after_seconds: STATE_TTL_SECONDS,
            }));
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(error = %e, "redis down; skipping oauth init throttle");
        }
    }

    // Fresh one-time state + nonce, stored server-side (short TTL).
    let state_token = sessions::generate_token();
    let nonce = uuid::Uuid::new_v4().to_string();
    state
        .redis
        .set_ex(
            &format!("{OAUTH_STATE_PREFIX}{state_token}"),
            &nonce,
            STATE_TTL_SECONDS,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to store oauth state");
            ApiError::new(ApiErrorKind::Internal(anyhow::Error::new(e)))
        })?;

    let url = state.oidc.authorize_url(&state_token, &nonce);
    if url.is_empty() {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "Google OAuth is not configured".into(),
        )));
    }
    Ok(Redirect::to(&url))
}

// ---------------------------------------------------------------------------
// OAuth callback (browser navigation)
// ---------------------------------------------------------------------------

async fn google_callback(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let ip = request_ip(&headers, Some(addr));

    // Login-abuse throttle (short-lived, fail-open as above).
    let cb_key = format!("{AUTH_CALLBACK_KEY_PREFIX}{ip}");
    match state
        .redis
        .increment_window(&cb_key, STATE_TTL_SECONDS)
        .await
    {
        Ok(count) if count > state.config.oauth_callback_limit_per_10_min => {
            tracing::warn!(ip = %ip, count, "oauth callback throttled");
            return redirect_frontend(&state.config, "/login?error=rate_limited");
        }
        _ => {}
    }

    if let Some(oauth_error) = params.get("error") {
        tracing::warn!(error = %oauth_error, "google returned an oauth error");
        return redirect_frontend(&state.config, "/login?error=denied");
    }

    let state_param = params.get("state").cloned().unwrap_or_default();
    let code = params.get("code").cloned().unwrap_or_default();

    // One-time state validation: consume and compare server-side.
    let nonce = match state
        .redis
        .getdel(&format!("{OAUTH_STATE_PREFIX}{state_param}"))
        .await
    {
        Ok(Some(nonce)) => nonce,
        Ok(None) => {
            tracing::warn!("oauth callback with unknown/expired state");
            return redirect_frontend(&state.config, "/login?error=invalid_state");
        }
        Err(e) => {
            tracing::error!(error = %e, "redis failure reading oauth state");
            return redirect_frontend(&state.config, "/login?error=service_unavailable");
        }
    };

    let identity = match state.oidc.exchange_code(&code, &nonce).await {
        Ok(identity) => identity,
        Err(e) => {
            tracing::warn!(error = %e, "oauth code exchange/verification failed");
            return redirect_frontend(&state.config, "/login?error=auth_failed");
        }
    };

    // Find or create the account.
    let user = match users::find_or_create_user(&state.db, &state.config, &identity).await {
        Ok((user, _created)) => user,
        Err(e) => {
            tracing::error!(error = %e, "failed to load or create user");
            return redirect_frontend(&state.config, "/login?error=service_unavailable");
        }
    };

    if user.status != "active" {
        tracing::info!(user_id = %user.id, "disabled user attempted login");
        return redirect_frontend(&state.config, "/login?error=disabled");
    }

    // Create a server-side session and hand the browser an HttpOnly cookie.
    let token = sessions::generate_token();
    let session = match sessions::create_session(
        &state.db,
        user.id,
        &token,
        state.config.session_ttl_seconds,
        Some(&hash_ip(&ip)),
        None,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "failed to create session");
            return redirect_frontend(&state.config, "/login?error=service_unavailable");
        }
    };

    tracing::info!(
        user_id = %user.id,
        session_id = %session,
        "login complete"
    );

    let jar = CookieJar::new().add(build_session_cookie(&state.config, &token));
    (
        jar,
        Redirect::to(&format!("{}/app", state.config.frontend_url)),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Logout / session lifecycle
// ---------------------------------------------------------------------------

async fn logout(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(token) = jar
        .get(&state.config.session_cookie_name)
        .map(|c| c.value().to_string())
    {
        let _ = sessions::revoke_session(&state.db, &token).await;
    }
    let jar = jar.remove(remove_session_cookie(&state.config));
    (jar, Json(json!({"ok": true}))).into_response()
}

async fn logout_all(
    State(state): State<AppState>,
    jar: CookieJar,
    user: middleware::UserContext,
) -> Response {
    if let Ok(_) = sessions::revoke_all_sessions(&state.db, user.user_id).await {
        tracing::info!(user_id = %user.user_id, "all sessions revoked");
    }
    let jar = jar.remove(remove_session_cookie(&state.config));
    (jar, Json(json!({"ok": true}))).into_response()
}

async fn me(State(state): State<AppState>, jar: CookieJar) -> Json<serde_json::Value> {
    let Some(token) = jar
        .get(&state.config.session_cookie_name)
        .map(|c| c.value().to_string())
    else {
        return Json(json!({ "authenticated": false }));
    };

    match sessions::lookup_session(&state.db, &token).await {
        Ok(Some(su))
            if su.revoked_at.is_none()
                && su.expires_at > chrono::Utc::now()
                && su.status == "active" =>
        {
            Json(json!({
                "authenticated": true,
                "user": {
                    "id": su.user_id,
                    "email": su.email,
                    "name": su.name,
                    "avatar_url": su.avatar_url,
                    "role": su.role_name,
                }
            }))
        }
        _ => Json(json!({ "authenticated": false })),
    }
}

// ---------------------------------------------------------------------------
// Dev/test-only stub login
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TestLoginRequest {
    email: String,
    name: Option<String>,
}

/// Creates a real session for an arbitrary email. Enabled only when
/// `AUTH_STUB=true` (see `.env.example`). Never enable in production.
async fn test_login(
    State(state): State<AppState>,
    Json(req): Json<TestLoginRequest>,
) -> Result<Response, ApiError> {
    if !state.config.auth_stub {
        return Err(ApiError::new(ApiErrorKind::NotFound));
    }
    if !req.email.contains('@') {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "email must be valid".into(),
        )));
    }

    let identity = OidcIdentity {
        subject: format!("stub:{}", req.email.to_lowercase()),
        email: req.email,
        name: req.name,
        picture: None,
    };
    let user = users::find_or_create_user(&state.db, &state.config, &identity)
        .await
        .map_err(ApiError::from)?
        .0;

    if user.status != "active" {
        return Err(ApiError::new(ApiErrorKind::Unauthorized));
    }

    let token = sessions::generate_token();
    sessions::create_session(
        &state.db,
        user.id,
        &token,
        state.config.session_ttl_seconds,
        Some(&hash_ip("stub")),
        None,
    )
    .await
    .map_err(ApiError::from)?;

    tracing::info!(user_id = %user.id, "stub login");

    let jar = CookieJar::new().add(build_session_cookie(&state.config, &token));
    let body = json!({
        "authenticated": true,
        "user": {
            "id": user.id,
            "email": user.email,
            "name": user.name,
            "avatar_url": user.avatar_url,
            "role": user.role_name,
        }
    });
    Ok((jar, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::config::Config;
    use crate::db;
    use crate::llm::OpenCodeClient;
    use crate::redis_store::Redis;
    use crate::state::AppState;

    /// Build a test state + router. Returns None when the live dependencies are
    /// unavailable, which makes the test a no-op outside a configured dev env.
    async fn test_app() -> Option<(Router, std::sync::MutexGuard<'static, ()>)> {
        let guard = crate::testutil::db_lock::LOCK.lock().unwrap();
        let db_url = std::env::var("DATABASE_URL").ok()?;
        let redis_url = std::env::var("REDIS_URL").ok()?;
        let pool = db::connect(&db_url).await.ok()?;
        let redis = Redis::connect(&redis_url).await.ok()?;

        let mut config = Config::from_env();
        config.auth_stub = true;
        config.session_cookie_name = "weave_session".to_string();

        let state = AppState {
            config: std::sync::Arc::new(config),
            llm: std::sync::Arc::new(OpenCodeClient::from_env()),
            db: pool,
            redis,
            oidc: std::sync::Arc::new(oauth::MockOidc::new(
                "https://idp.example/authorize",
                oauth::OidcIdentity {
                    subject: "sub-test".to_string(),
                    email: "integration@example.com".to_string(),
                    name: Some("Integration".to_string()),
                    picture: None,
                },
            )),
        };
        Some((routes().with_state(state), guard))
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn stub_login_me_logout_flow() {
        let Some((app, _guard)) = test_app().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };

        // me without a session -> unauthenticated
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), axum::http::StatusCode::OK);
        assert_eq!(body_json(res).await["authenticated"], false);

        // stub login -> session cookie + member role
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/test/login")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"email":"integration@example.com"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), axum::http::StatusCode::OK);
        let cookie = res
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(cookie.contains("weave_session="));
        let me = body_json(res).await;
        assert_eq!(me["authenticated"], true);
        assert_eq!(me["user"]["email"], "integration@example.com");
        assert_eq!(me["user"]["role"], "member");

        // me with the cookie -> authenticated
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me = body_json(res).await;
        assert_eq!(me["authenticated"], true);
        assert_eq!(me["user"]["email"], "integration@example.com");

        // logout revokes the session
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/logout")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), axum::http::StatusCode::OK);

        // me with the revoked cookie -> unauthenticated
        let res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(res).await["authenticated"], false);
    }
}
