//! Admin pipeline tracer: observe the `/api/graph/ingest` pipeline end-to-end.
//!
//! Every stage a live ingest goes through is reproduced here by calling the
//! *same* functions as the graph middleware and handlers (`graph::enforce`,
//! `ratelimit::*`, `weave_core::extract::extract_delta_traced`), so the trace
//! reflects exactly what a real request does.
//!
//! Observe-only: rate-limit / quota / concurrency checks run and their outcome
//! is reported, but they never abort the trace — the LLM stage always runs so
//! an admin can inspect the exact prompt and raw response.

use std::time::Instant;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::analytics;
use crate::auth::middleware::{UserContext, require_permission};
use crate::error::{ApiError, ApiErrorKind};
use crate::ratelimit;
use crate::state::AppState;
use crate::usage::{self, UsageRecord};
use weave_core::extract::extract_delta_traced;
use weave_core::models::IngestRequest;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/admin/pipeline/ingest", post(ingest_trace))
}

/// One pipeline step: what ran, whether it would have blocked, how long it
/// took, and the step's input/output as JSON.
#[derive(Serialize)]
struct Stage {
    stage: &'static str,
    status: &'static str,
    duration_ms: i64,
    detail: Value,
}

impl Stage {
    fn ok(stage: &'static str, start: Instant, detail: Value) -> Self {
        Self {
            stage,
            status: "ok",
            duration_ms: now_ms(start),
            detail,
        }
    }

    fn error(stage: &'static str, start: Instant, message: String) -> Self {
        Self {
            stage,
            status: "error",
            duration_ms: now_ms(start),
            detail: json!({ "error": message }),
        }
    }
}

fn now_ms(start: Instant) -> i64 {
    start.elapsed().as_millis().try_into().unwrap_or(0)
}

async fn ingest_trace(
    State(state): State<AppState>,
    user: UserContext,
    Json(req): Json<IngestRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.pipeline.read")?;

    if req.text.trim().is_empty() {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "text must not be empty".into(),
        )));
    }

    let mut stages: Vec<Stage> = Vec::new();
    let total = Instant::now();

    // -- auth --------------------------------------------------------------
    // Resolved by the `UserContext` extractor: session -> user -> role.
    let start = Instant::now();
    let mut perms: Vec<String> = user.permissions.iter().cloned().collect();
    perms.sort();
    stages.push(Stage::ok(
        "auth",
        start,
        json!({
            "user_id": user.user_id,
            "email": user.email,
            "role": user.role_name,
            "status": "active",
            "permissions": perms,
        }),
    ));

    // -- permission --------------------------------------------------------
    let start = Instant::now();
    match require_permission(&user, "graph.ingest") {
        Ok(()) => stages.push(Stage::ok(
            "permission",
            start,
            json!({ "required": "graph.ingest", "granted": true }),
        )),
        Err(_) => stages.push(Stage::ok(
            "permission",
            start,
            json!({ "required": "graph.ingest", "granted": false, "would_return": 403 }),
        )),
    }

    // -- policy resolution -------------------------------------------------
    let start = Instant::now();
    let resolved = match ratelimit::resolve_limits(&state.db, &state.config, &user, "graph.ingest")
        .await
    {
        Ok(limits) => {
            stages.push(Stage::ok(
                "policy",
                start,
                json!({
                    "scope": "global -> role -> user override, clamped to hard ceiling",
                    "role": user.role_name,
                    "effective_limits": limits,
                }),
            ));
            limits
        }
        Err(e) => {
            stages.push(Stage::error(
                "policy",
                start,
                format!("policy resolution failed: {e}"),
            ));
            return Err(ApiError::from(e));
        }
    };

    // -- rate limit --------------------------------------------------------
    // Observe-only: the real atomic counter check runs; a hit is reported but
    // never blocks the trace.
    let start = Instant::now();
    match ratelimit::check_request_limits(&state.redis, user.user_id, "graph.ingest", &resolved)
        .await
    {
        Ok(retry_after) => stages.push(Stage::ok(
            "rate_limit",
            start,
            json!({
                "observe_only": true,
                "would_block": retry_after.is_some(),
                "retry_after_seconds": retry_after,
                "effective_limits": resolved,
            }),
        )),
        Err(e) => stages.push(Stage::error(
            "rate_limit",
            start,
            format!("redis unavailable: {e}"),
        )),
    };

    // -- token quota -------------------------------------------------------
    let start = Instant::now();
    match ratelimit::check_token_limits(&state.redis, user.user_id, &resolved).await {
        Ok(exhausted) => stages.push(Stage::ok(
            "quota",
            start,
            json!({
                "observe_only": true,
                "exhausted": exhausted,
                "tokens_per_day": resolved.tokens_per_day,
                "tokens_per_month": resolved.tokens_per_month,
            }),
        )),
        Err(e) => stages.push(Stage::error(
            "quota",
            start,
            format!("redis unavailable: {e}"),
        )),
    };

    // -- concurrency slot --------------------------------------------------
    let start = Instant::now();
    let concurrency = ratelimit::acquire_concurrency(&state.redis, user.user_id, "graph.ingest", &resolved)
        .await;
    let _slot = match concurrency {
        Ok(ratelimit::ConcurrencyResult::Acquired(guard)) => {
            stages.push(Stage::ok(
                "concurrency",
                start,
                json!({
                    "observe_only": true,
                    "configured": true,
                    "acquired": true,
                    "limit": resolved.concurrent_requests,
                }),
            ));
            Some(guard)
        }
        Ok(ratelimit::ConcurrencyResult::Limited) => {
            stages.push(Stage::ok(
                "concurrency",
                start,
                json!({
                    "observe_only": true,
                    "configured": true,
                    "acquired": false,
                    "limit": resolved.concurrent_requests,
                    "would_return": 429,
                }),
            ));
            None
        }
        Ok(ratelimit::ConcurrencyResult::NotConfigured) => {
            stages.push(Stage::ok(
                "concurrency",
                start,
                json!({ "observe_only": true, "configured": false }),
            ));
            None
        }
        Err(e) => {
            stages.push(Stage::error(
                "concurrency",
                start,
                format!("redis unavailable: {e}"),
            ));
            None
        }
    };

    // -- LLM extraction ----------------------------------------------------
    let start = Instant::now();
    let (delta, usage, trace) = extract_delta_traced(&state.llm, &req).await;
    let extract_status = if trace.mock_fallback { "mock" } else { "ok" };
    stages.push(Stage {
        stage: "extract",
        status: extract_status,
        duration_ms: now_ms(start),
        detail: serde_json::to_value(&trace).unwrap_or_else(|_| json!({ "error": "serialize trace" })),
    });

    // -- usage + analytics -------------------------------------------------
    let start = Instant::now();
    usage::record_and_count(
        &state.db,
        &state.redis,
        UsageRecord {
            user_id: user.user_id,
            request_id: Uuid::parse_str(&user.request_id).ok(),
            endpoint: "graph.ingest",
            provider: Some("opencode".to_string()),
            model: Some(state.llm.model.clone()),
            usage,
            latency_ms: Some(now_ms(total)),
            status_code: 200,
        },
    )
    .await;
    analytics::record_spawn(
        &state.db,
        analytics::AnalyticsEvent {
            user_id: Some(user.user_id),
            event_type: analytics::event_for_endpoint("graph.ingest"),
            request_id: Uuid::parse_str(&user.request_id).ok(),
            endpoint: Some("graph.ingest"),
            metadata: None,
        },
    );
    stages.push(Stage::ok(
        "usage",
        start,
        json!({ "usage_recorded": true, "analytics_recorded": true }),
    ));

    // -- response ----------------------------------------------------------
    let start = Instant::now();
    stages.push(Stage::ok(
        "response",
        start,
        json!({ "status_code": 200, "body": delta }),
    ));

    Ok(Json(json!({
        "endpoint": "graph.ingest",
        "llm_mode": if state.llm.available() { "opencode" } else { "mock" },
        "total_ms": now_ms(total),
        "stages": stages,
        "delta": delta,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::auth::oauth::{MockOidc, OidcIdentity};
    use crate::config::Config;
    use crate::db;
    use crate::redis_store::Redis;
    use weave_core::llm::OpenCodeClient;

    struct TestCtx {
        app: Router,
        db: sqlx::PgPool,
    }

    /// App with auth + admin routes and a mock LLM (no API key), so the
    /// pipeline tracer runs its deterministic mock extractor instead of making
    /// a live provider call.
    async fn setup() -> Option<(TestCtx, std::sync::MutexGuard<'static, ()>)> {
        let guard = crate::testutil::db_lock::LOCK.lock().unwrap();
        let db_url = std::env::var("DATABASE_URL").ok()?;
        let redis_url = std::env::var("REDIS_URL").ok()?;
        let pool = db::connect(&db_url).await.ok()?;
        let redis = Redis::connect(&redis_url).await.ok()?;
        let mut config = Config::from_env();
        config.auth_stub = true;
        let state = AppState {
            config: std::sync::Arc::new(config),
            llm: std::sync::Arc::new(OpenCodeClient::mock()),
            db: pool.clone(),
            redis,
            oidc: std::sync::Arc::new(MockOidc::new(
                "https://idp.example/a",
                OidcIdentity {
                    subject: "sub-pipeline-test".to_string(),
                    email: "admin@test.com".to_string(),
                    name: None,
                    picture: None,
                },
            )),
        };
        let app = crate::auth::routes().merge(crate::admin::routes()).with_state(state);
        Some((TestCtx { app, db: pool }, guard))
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

    async fn run_pipeline(
        app: &Router,
        cookie: &str,
        body: &str,
    ) -> (StatusCode, serde_json::Value) {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/admin/pipeline/ingest")
                    .header("cookie", cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 128 * 1024).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, value)
    }

    #[tokio::test]
    async fn admin_sees_full_pipeline() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let admin_cookie = login(&ctx.app, "admin@test.com").await;
        sqlx::query(
            "UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') WHERE email = $1",
        )
        .bind("admin@test.com")
        .execute(&ctx.db)
        .await
        .unwrap();

        let (status, body) = run_pipeline(
            &ctx.app,
            &admin_cookie,
            r#"{"text":"Hogwarts has four houses.","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "admin must be able to run the pipeline");

        let stages = body["stages"].as_array().expect("stages array present");
        let names: Vec<&str> = stages
            .iter()
            .filter_map(|s| s["stage"].as_str())
            .collect();
        for expected in ["auth", "permission", "policy", "rate_limit", "quota", "concurrency", "extract", "usage", "response"] {
            assert!(
                names.contains(&expected),
                "pipeline must include {expected} stage, got {names:?}"
            );
        }

        // Mock extractor: trace records the fallback and the exact prompts.
        let extract = stages
            .iter()
            .find(|s| s["stage"] == "extract")
            .unwrap();
        assert_eq!(extract["status"], "mock");
        assert_eq!(extract["detail"]["mock_fallback"], true);
        assert!(
            extract["detail"]["user_prompt"]
                .as_str()
                .unwrap()
                .contains("Hogwarts has four houses."),
            "user prompt must carry the new note"
        );
        assert!(
            extract["detail"]["system_prompt"].as_str().unwrap().contains("knowledge-graph extractor"),
            "system prompt must be the extractor prompt"
        );

        // The response body is echoed, and the delta carries the mock output.
        assert!(body["delta"].is_object());
    }

    #[tokio::test]
    async fn member_is_forbidden() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let member_cookie = login(&ctx.app, "member@test.com").await;

        let (status, _) = run_pipeline(
            &ctx.app,
            &member_cookie,
            r#"{"text":"x","nodes":[],"edges":[]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "members must be denied");
    }
}