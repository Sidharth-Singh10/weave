//! Analytics event recording.
//!
//! Events are coarse-grained product/operational signals — never raw graph
//! contents, prompts, or responses. Recording is fire-and-forget so a
//! metering hiccup cannot break the core operation.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub mod event_type {
    pub const LOGIN: &str = "login";
    pub const LOGOUT: &str = "logout";
    // Reserved for future session-creation events.
    #[allow(dead_code)]
    pub const SESSION_STARTED: &str = "session_started";
    pub const GRAPH_INGEST: &str = "graph_ingest";
    pub const GRAPH_ORGANIZE: &str = "graph_organize";
    pub const GRAPH_SEARCH: &str = "graph_search";
    pub const COMMUNITY_LABELED: &str = "community_labeled";
    pub const RATE_LIMIT_HIT: &str = "rate_limit_hit";
    pub const QUOTA_EXCEEDED: &str = "quota_exceeded";
}

pub struct AnalyticsEvent {
    pub user_id: Option<Uuid>,
    pub event_type: &'static str,
    pub request_id: Option<Uuid>,
    pub endpoint: Option<&'static str>,
    pub metadata: Option<Value>,
}

/// Insert an analytics event (returns the DB error for callers that need it).
pub async fn record(pool: &PgPool, event: AnalyticsEvent) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO analytics_events (user_id, event_type, request_id, endpoint, metadata)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(event.user_id)
    .bind(event.event_type)
    .bind(event.request_id)
    .bind(event.endpoint)
    .bind(event.metadata)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fire-and-forget recording: spawned, logged on failure, never blocks the
/// request path.
pub fn record_spawn(pool: &PgPool, event: AnalyticsEvent) {
    let pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = record(&pool, event).await {
            tracing::warn!(error = %e, "analytics recording failed");
        }
    });
}

/// Map a graph policy endpoint to its analytics event type.
pub fn event_for_endpoint(endpoint: &str) -> &'static str {
    match endpoint {
        "graph.ingest" => event_type::GRAPH_INGEST,
        "graph.organize" => event_type::GRAPH_ORGANIZE,
        "graph.search" => event_type::GRAPH_SEARCH,
        "graph.label_community" => event_type::COMMUNITY_LABELED,
        _ => "graph_operation",
    }
}
