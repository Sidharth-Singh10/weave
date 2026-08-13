//! Graph API routes with authentication, permission, and rate-limit
//! enforcement.
//!
//! Pipeline per request (as specified): auth → permission → resolve policy →
//! rate limit request → concurrency slot → handler → release slot. Redis
//! failures on these LLM endpoints fail closed (503) rather than allowing
//! unlimited LLM traffic.

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::routing::post;
use axum::{Json, Router};

use crate::analytics::{self, event_type};
use crate::auth::middleware::{AuthUser, UserContext, require_permission};
use crate::error::{ApiError, ApiErrorKind};
use crate::models::{
    GraphDelta, IngestRequest, LabelCommunityRequest, LabelCommunityResult, OrganizeRequest,
    OrganizeResult, SearchRequest, SearchResult,
};
use crate::ratelimit;
use crate::state::AppState;
use crate::usage::{self, UsageRecord};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/graph/ingest", post(ingest))
        .route("/api/graph/organize", post(organize_graph))
        .route("/api/graph/search", post(search_graph))
        .route("/api/graph/label-community", post(label_community_graph))
}

/// Maps a request path to the permission key / policy endpoint identifier.
fn endpoint_for_path(path: &str) -> Option<&'static str> {
    match path {
        "/api/graph/ingest" => Some("graph.ingest"),
        "/api/graph/organize" => Some("graph.organize"),
        "/api/graph/search" => Some("graph.search"),
        "/api/graph/label-community" => Some("graph.label_community"),
        _ => None,
    }
}

/// Middleware: auth → permission → policy → rate limit → concurrency.
pub async fn enforce(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    let Some(endpoint) = endpoint_for_path(request.uri().path()) else {
        return Ok(next.run(request).await);
    };

    let (mut parts, body) = request.into_parts();

    // Auth (session → user → role → permissions), same logic as the
    // `UserContext` extractor.
    use axum::extract::FromRequestParts;
    let user =
        <UserContext as FromRequestParts<AppState>>::from_request_parts(&mut parts, &state).await?;
    require_permission(&user, endpoint)?;

    // Effective policy for this user + endpoint.
    let resolved = ratelimit::resolve_limits(&state.db, &state.config, &user, endpoint)
        .await
        .map_err(ApiError::from)?;

    // Request-window counters (atomic Lua, fail closed on Redis errors).
    let blocked = ratelimit::check_request_limits(&state.redis, user.user_id, endpoint, &resolved)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "redis unavailable; failing closed for {endpoint}");
            ApiError::new(ApiErrorKind::ServiceUnavailable(
                "rate limiter temporarily unavailable".into(),
            ))
            .with_request_id(Some(user.request_id.clone()))
        })?;
    if let Some(retry_after_seconds) = blocked {
        record_event(&state, &user, event_type::RATE_LIMIT_HIT, endpoint);
        return Err(ApiError::new(ApiErrorKind::RateLimitExceeded {
            retry_after_seconds,
        })
        .with_request_id(Some(user.request_id.clone())));
    }

    // Token quota (checked before the expensive LLM call).
    let quota_exhausted = ratelimit::check_token_limits(&state.redis, user.user_id, &resolved)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "redis unavailable; failing closed for {endpoint}");
            ApiError::new(ApiErrorKind::ServiceUnavailable(
                "rate limiter temporarily unavailable".into(),
            ))
            .with_request_id(Some(user.request_id.clone()))
        })?;
    if quota_exhausted {
        record_event(&state, &user, event_type::QUOTA_EXCEEDED, endpoint);
        return Err(ApiError::new(ApiErrorKind::QuotaExceeded)
            .with_request_id(Some(user.request_id.clone())));
    }

    // Concurrency slot held for the duration of the handler.
    let _concurrency =
        ratelimit::acquire_concurrency(&state.redis, user.user_id, endpoint, &resolved)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "redis unavailable; failing closed for {endpoint}");
                ApiError::new(ApiErrorKind::ServiceUnavailable(
                    "rate limiter temporarily unavailable".into(),
                ))
                .with_request_id(Some(user.request_id.clone()))
            })?;

    parts.extensions.insert(user);
    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn ingest(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<IngestRequest>,
) -> Result<Json<GraphDelta>, ApiError> {
    if req.text.trim().is_empty() {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "text must not be empty".into(),
        )));
    }
    let start = std::time::Instant::now();
    let (delta, usage) = crate::extract::extract_delta(&state.llm, &req).await;
    record_usage(&state, &user, "graph.ingest", usage, start, 200).await;
    Ok(Json(delta))
}

async fn organize_graph(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<OrganizeRequest>,
) -> Result<Json<OrganizeResult>, ApiError> {
    let start = std::time::Instant::now();
    let (result, usage) = crate::organize::organize(&state.llm, &req).await;
    record_usage(&state, &user, "graph.organize", usage, start, 200).await;
    Ok(Json(result))
}

async fn search_graph(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResult>, ApiError> {
    let start = std::time::Instant::now();
    let (result, usage) = crate::organize::search(&state.llm, &req).await;
    record_usage(&state, &user, "graph.search", usage, start, 200).await;
    Ok(Json(result))
}

async fn label_community_graph(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(req): Json<LabelCommunityRequest>,
) -> Result<Json<LabelCommunityResult>, ApiError> {
    let start = std::time::Instant::now();
    let (result, usage) = crate::organize::label_community(&state.llm, &req).await;
    record_usage(&state, &user, "graph.label_community", usage, start, 200).await;
    Ok(Json(result))
}

/// Best-effort usage metering: persist a usage row and feed token counters.
/// Never fails the request.
async fn record_usage(
    state: &AppState,
    user: &UserContext,
    endpoint: &'static str,
    usage: Option<crate::llm::TokenUsage>,
    start: std::time::Instant,
    status: i32,
) {
    usage::record_and_count(
        &state.db,
        &state.redis,
        UsageRecord {
            user_id: user.user_id,
            request_id: uuid::Uuid::parse_str(&user.request_id).ok(),
            endpoint,
            provider: Some("opencode".to_string()),
            model: Some(state.llm.model.clone()),
            usage,
            latency_ms: Some(usage::now_ms(start)),
            status_code: status,
        },
    )
    .await;
    record_event(
        state,
        user,
        analytics::event_for_endpoint(endpoint),
        endpoint,
    );
}

/// Fire-and-forget analytics event with the request id attached.
fn record_event(state: &AppState, user: &UserContext, event: &'static str, endpoint: &'static str) {
    analytics::record_spawn(
        &state.db,
        analytics::AnalyticsEvent {
            user_id: Some(user.user_id),
            event_type: event,
            request_id: uuid::Uuid::parse_str(&user.request_id).ok(),
            endpoint: Some(endpoint),
            metadata: None,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::middleware;
    use tower::ServiceExt;

    use crate::auth::oauth::MockOidc;
    use crate::config::Config;
    use crate::db;
    use crate::llm::OpenCodeClient;
    use crate::redis_store::Redis;

    struct TestCtx {
        app: Router,
        db: sqlx::PgPool,
        redis: Redis,
    }

    async fn setup() -> Option<(TestCtx, std::sync::MutexGuard<'static, ()>)> {
        let guard = crate::testutil::db_lock::LOCK.lock().unwrap();
        let db_url = std::env::var("DATABASE_URL").ok()?;
        let redis_url = std::env::var("REDIS_URL").ok()?;
        let pool = db::connect(&db_url).await.ok()?;
        let redis = Redis::connect(&redis_url).await.ok()?;
        let mut config = Config::test_default();
        config.auth_stub = true;
        let state = AppState {
            config: std::sync::Arc::new(config),
            llm: std::sync::Arc::new(OpenCodeClient::from_env()),
            db: pool.clone(),
            redis: redis.clone(),
            oidc: std::sync::Arc::new(MockOidc::new(
                "https://idp.example/a",
                crate::auth::oauth::OidcIdentity {
                    subject: "sub-rl-test".to_string(),
                    email: "rl@test.com".to_string(),
                    name: None,
                    picture: None,
                },
            )),
        };
        let app = crate::auth::routes()
            .merge(routes().layer(middleware::from_fn_with_state(state.clone(), enforce)))
            .with_state(state);
        Some((
            TestCtx {
                app,
                db: pool,
                redis,
            },
            guard,
        ))
    }

    async fn login(app: &Router, email: &str) -> String {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/test/login")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"email":"{email}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        cookie.split(';').next().unwrap().to_string()
    }

    async fn post(
        app: &Router,
        path: &str,
        cookie: &str,
        body: &str,
    ) -> (StatusCode, Option<String>) {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("cookie", cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let retry = res
            .headers()
            .get("retry-after")
            .map(|v| v.to_str().unwrap().to_string());
        let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).ok();
        (status, text.or(retry))
    }

    async fn user_id(db: &sqlx::PgPool, email: &str) -> uuid::Uuid {
        let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(db)
            .await
            .unwrap();
        id
    }

    /// Clear the test user's request counters so repeated runs stay deterministic.
    async fn clear_counters(redis: &Redis, user_id: uuid::Uuid) {
        let mut conn = redis.connection();
        for endpoint in [
            "graph.ingest",
            "graph.search",
            "graph.organize",
            "graph.label_community",
        ] {
            let key = format!("rl:{user_id}:{endpoint}:requests:minute");
            let _ = redis::cmd("DEL")
                .arg(&key)
                .query_async::<()>(&mut conn)
                .await;
        }
    }

    /// Create (or replace) a user override policy with the given limits.
    async fn set_user_override(
        db: &sqlx::PgPool,
        user_id: uuid::Uuid,
        endpoint: Option<&str>,
        limits: &crate::policy::Limits,
    ) {
        let policy_id: (uuid::Uuid,) = sqlx::query_as(
            r#"INSERT INTO rate_limit_policies (scope_type, user_id, endpoint)
               VALUES ('user', $1, $2)
               ON CONFLICT (scope_type, user_id, (COALESCE(endpoint, ''))) WHERE scope_type = 'user'
               DO UPDATE SET updated_at = now()
               RETURNING id"#,
        )
        .bind(user_id)
        .bind(endpoint)
        .fetch_one(db)
        .await
        .unwrap();
        sqlx::query("DELETE FROM rate_limit_rules WHERE policy_id = $1")
            .bind(policy_id.0)
            .execute(db)
            .await
            .unwrap();
        for rule in limits.rules() {
            sqlx::query(
                "INSERT INTO rate_limit_rules (policy_id, metric, time_window, limit_value)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(policy_id.0)
            .bind(rule.metric)
            .bind(rule.time_window)
            .bind(rule.limit)
            .execute(db)
            .await
            .unwrap();
        }
    }

    async fn clear_user_overrides(db: &sqlx::PgPool, user_id: uuid::Uuid) {
        sqlx::query("DELETE FROM rate_limit_policies WHERE scope_type='user' AND user_id=$1")
            .bind(user_id)
            .execute(db)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn endpoint_specific_policy_enforces() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let cookie = login(&ctx.app, "rl@test.com").await;
        let id = user_id(&ctx.db, "rl@test.com").await;
        clear_counters(&ctx.redis, id).await;

        // Endpoint-specific user override: search = 1/min, ingest unaffected.
        let limits = crate::policy::Limits {
            requests_per_minute: Some(1),
            ..Default::default()
        };
        set_user_override(&ctx.db, id, Some("graph.search"), &limits).await;

        // First search passes.
        let (status, _) = post(
            &ctx.app,
            "/api/graph/search",
            &cookie,
            r#"{"query":"x","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "first search should pass");
        // Second search is rate limited.
        let (status, _) = post(
            &ctx.app,
            "/api/graph/search",
            &cookie,
            r#"{"query":"x","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "second search should be 429"
        );
        // A different endpoint (ingest) is unaffected by the search override.
        let (status, _) = post(
            &ctx.app,
            "/api/graph/ingest",
            &cookie,
            r#"{"text":"x","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "ingest must not inherit the search limit"
        );

        clear_user_overrides(&ctx.db, id).await;
    }

    #[tokio::test]
    async fn user_override_and_quota() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let cookie = login(&ctx.app, "rl@test.com").await;
        let id = user_id(&ctx.db, "rl@test.com").await;
        clear_counters(&ctx.redis, id).await;

        // Generic user override: 2 requests/min across all graph endpoints.
        let limits = crate::policy::Limits {
            requests_per_minute: Some(2),
            ..Default::default()
        };
        set_user_override(&ctx.db, id, None, &limits).await;

        // Two allowed, third blocked.
        for _ in 0..2 {
            let (status, _) = post(
                &ctx.app,
                "/api/graph/ingest",
                &cookie,
                r#"{"text":"x","nodes":[],"edges":[]}"#,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, retry) = post(
            &ctx.app,
            "/api/graph/ingest",
            &cookie,
            r#"{"text":"x","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(retry.is_some(), "429 must include a retry hint");

        clear_user_overrides(&ctx.db, id).await;
    }

    #[tokio::test]
    async fn usage_is_recorded() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let cookie = login(&ctx.app, "rl@test.com").await;
        let id = user_id(&ctx.db, "rl@test.com").await;
        clear_counters(&ctx.redis, id).await;

        // Remove prior usage rows for the shared test user.
        sqlx::query("DELETE FROM usage_events WHERE user_id = $1")
            .bind(id)
            .execute(&ctx.db)
            .await
            .unwrap();

        let (status, _) = post(
            &ctx.app,
            "/api/graph/ingest",
            &cookie,
            r#"{"text":"Hogwarts has four houses.","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM usage_events WHERE user_id = $1 AND endpoint = 'graph.ingest'",
        )
        .bind(id)
        .fetch_one(&ctx.db)
        .await
        .unwrap();
        assert_eq!(count, 1, "a usage event must be recorded for the ingest");

        let (endpoint, latency_ms, status_code): (String, Option<i64>, i32) = sqlx::query_as(
            "SELECT endpoint, latency_ms, status_code FROM usage_events WHERE user_id = $1 AND endpoint = 'graph.ingest'",
        )
        .bind(id)
        .fetch_one(&ctx.db)
        .await
        .unwrap();
        assert_eq!(endpoint, "graph.ingest");
        assert!(latency_ms.is_some(), "latency must be recorded");
        assert_eq!(status_code, 200);
    }
}
