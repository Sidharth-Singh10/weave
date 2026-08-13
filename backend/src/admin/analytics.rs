//! Admin analytics aggregation endpoints.
//!
//! Aggregates are computed server-side with indexed grouped queries — the
//! browser never receives raw event rows.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::middleware::{UserContext, require_permission};
use crate::error::{ApiError, ApiErrorKind};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/analytics/overview", get(overview))
        .route("/api/admin/analytics/users/{id}", get(user_analytics))
}

#[derive(Debug, Deserialize)]
pub struct OverviewQuery {
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_days() -> i64 {
    30
}

fn since_days(days: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() - chrono::Duration::days(days.clamp(1, 365))
}

/// Daily series helper: fill every day in [since, today] with a value.
async fn daily_series(
    pool: &sqlx::PgPool,
    since: chrono::DateTime<chrono::Utc>,
    sql: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query(sql).bind(since).fetch_all(pool).await?;
    Ok(rows
        .iter()
        .map(|row| {
            json!({
                "date": row.get::<chrono::NaiveDate, _>("day"),
                "value": row.get::<i64, _>("value"),
            })
        })
        .collect())
}

async fn overview(
    State(state): State<AppState>,
    user: UserContext,
    Query(q): Query<OverviewQuery>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.analytics.read")?;
    let since = since_days(q.days);

    // -- Totals -----------------------------------------------------------
    let totals = sqlx::query(
        r#"
        SELECT
          (SELECT count(*) FROM users)::bigint AS total_users,
          (SELECT count(*) FROM users WHERE status = 'active')::bigint AS active_users,
          (SELECT count(*) FROM users WHERE created_at >= $1)::bigint AS new_users,
          (SELECT count(*) FROM usage_events WHERE created_at >= date_trunc('day', now()))::bigint AS requests_today,
          (SELECT COALESCE(sum(total_tokens), 0) FROM usage_events WHERE created_at >= date_trunc('day', now()))::bigint AS tokens_today,
          (SELECT count(*) FROM analytics_events WHERE event_type = 'rate_limit_hit' AND timestamp >= $1)::bigint AS rate_limit_hits
        "#,
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)?;

    // -- Charts -----------------------------------------------------------
    let active_users = daily_series(
        &state.db,
        since,
        r#"
        SELECT days.day, COALESCE(cnt, 0)::bigint AS value FROM
          (SELECT generate_series($1::date, current_date, '1 day')::date AS day) days
        LEFT JOIN (SELECT created_at::date AS day, count(DISTINCT user_id) AS cnt
                   FROM usage_events WHERE user_id IS NOT NULL GROUP BY 1) counts
          ON counts.day = days.day
        ORDER BY days.day
        "#,
    )
    .await
    .map_err(ApiError::from)?;

    let requests = daily_series(
        &state.db,
        since,
        r#"
        SELECT days.day, COALESCE(cnt, 0)::bigint AS value FROM
          (SELECT generate_series($1::date, current_date, '1 day')::date AS day) days
        LEFT JOIN (SELECT created_at::date AS day, count(*) AS cnt
                   FROM usage_events GROUP BY 1) counts
          ON counts.day = days.day
        ORDER BY days.day
        "#,
    )
    .await
    .map_err(ApiError::from)?;

    let llm_tokens = daily_series(
        &state.db,
        since,
        r#"
        SELECT days.day, COALESCE(sum(total_tokens), 0)::bigint AS value FROM
          (SELECT generate_series($1::date, current_date, '1 day')::date AS day) days
        LEFT JOIN (SELECT created_at::date AS day, sum(total_tokens) AS total_tokens
                   FROM usage_events GROUP BY 1) counts
          ON counts.day = days.day
        GROUP BY days.day
        ORDER BY days.day
        "#,
    )
    .await
    .map_err(ApiError::from)?;

    let new_users = daily_series(
        &state.db,
        since,
        r#"
        SELECT days.day, COALESCE(cnt, 0)::bigint AS value FROM
          (SELECT generate_series($1::date, current_date, '1 day')::date AS day) days
        LEFT JOIN (SELECT created_at::date AS day, count(*) AS cnt
                   FROM users GROUP BY 1) counts
          ON counts.day = days.day
        ORDER BY days.day
        "#,
    )
    .await
    .map_err(ApiError::from)?;

    // -- LLM metrics ------------------------------------------------------
    let llm_totals = sqlx::query(
        r#"
        SELECT COALESCE(sum(input_tokens), 0)::bigint AS input_tokens,
               COALESCE(sum(output_tokens), 0)::bigint AS output_tokens,
               COALESCE(sum(total_tokens), 0)::bigint AS total_tokens
        FROM usage_events WHERE created_at >= $1
        "#,
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)?;

    let by_model: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(model, 'unknown')::text, COALESCE(sum(total_tokens),0)::bigint FROM usage_events WHERE created_at >= $1 AND model IS NOT NULL GROUP BY model ORDER BY 2 DESC",
    )
    .bind(since)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    let by_endpoint: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT endpoint, count(*)::bigint, COALESCE(sum(total_tokens),0)::bigint FROM usage_events WHERE created_at >= $1 GROUP BY endpoint ORDER BY 2 DESC",
    )
    .bind(since)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    // -- API metrics ------------------------------------------------------
    let (avg_latency, p95_latency): (Option<f64>, Option<f64>) = sqlx::query_as(
        r#"
        SELECT avg(latency_ms)::float8,
               percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms)::float8
        FROM usage_events WHERE created_at >= $1 AND latency_ms IS NOT NULL
        "#,
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)?;

    // -- Top users --------------------------------------------------------
    let top_users: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT u.email, count(ue.id)::bigint AS requests, COALESCE(sum(ue.total_tokens),0)::bigint AS tokens
        FROM usage_events ue JOIN users u ON u.id = ue.user_id
        WHERE ue.created_at >= $1
        GROUP BY u.id, u.email ORDER BY tokens DESC LIMIT 10
        "#,
    )
    .bind(since)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(json!({
        "period_days": q.days,
        "totals": {
            "total_users": totals.get::<i64, _>("total_users"),
            "active_users": totals.get::<i64, _>("active_users"),
            "new_users": totals.get::<i64, _>("new_users"),
            "requests_today": totals.get::<i64, _>("requests_today"),
            "llm_tokens_today": totals.get::<i64, _>("tokens_today"),
            "rate_limit_hits": totals.get::<i64, _>("rate_limit_hits"),
        },
        "charts": {
            "active_users": active_users,
            "requests": requests,
            "llm_tokens": llm_tokens,
            "new_users": new_users,
        },
        "llm": {
            "input_tokens": llm_totals.get::<i64, _>("input_tokens"),
            "output_tokens": llm_totals.get::<i64, _>("output_tokens"),
            "total_tokens": llm_totals.get::<i64, _>("total_tokens"),
            "by_model": by_model.iter().map(|(m, t)| json!({"model": m, "tokens": t})).collect::<Vec<_>>(),
            "by_endpoint": by_endpoint.iter().map(|(e, r, t)| json!({"endpoint": e, "requests": r, "tokens": t})).collect::<Vec<_>>(),
        },
        "api": {
            "avg_latency_ms": avg_latency,
            "p95_latency_ms": p95_latency,
        },
        "top_users": top_users.iter().map(|(e, r, t)| json!({"email": e, "requests": r, "tokens": t})).collect::<Vec<_>>(),
    })))
}

async fn user_analytics(
    State(state): State<AppState>,
    user: UserContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.analytics.read")?;

    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(ApiError::from)?;
    if !exists {
        return Err(ApiError::new(ApiErrorKind::NotFound));
    }

    let (email,): (String,) = sqlx::query_as("SELECT email FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(ApiError::from)?;

    let (requests, tokens, last_request): (i64, i64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            r#"
            SELECT count(*)::bigint, COALESCE(sum(total_tokens), 0)::bigint, max(created_at)
            FROM usage_events WHERE user_id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(ApiError::from)?;

    let by_day: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        r#"
        SELECT created_at::date, count(*)::bigint, COALESCE(sum(total_tokens), 0)::bigint
        FROM usage_events WHERE user_id = $1 GROUP BY 1 ORDER BY 1 DESC LIMIT 30
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    let by_endpoint: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT endpoint, count(*)::bigint, COALESCE(sum(total_tokens), 0)::bigint
        FROM usage_events WHERE user_id = $1 GROUP BY endpoint ORDER BY 2 DESC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(json!({
        "user": { "id": id, "email": email },
        "totals": { "requests": requests, "tokens": tokens, "last_request": last_request },
        "by_day": by_day.iter().map(|(d, r, t)| json!({"date": d, "requests": r, "tokens": t})).collect::<Vec<_>>(),
        "by_endpoint": by_endpoint.iter().map(|(e, r, t)| json!({"endpoint": e, "requests": r, "tokens": t})).collect::<Vec<_>>(),
    })))
}
