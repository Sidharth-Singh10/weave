//! LLM usage metering: writes `usage_events` rows and feeds Redis token
//! counters (used by the quota check). Recording is best-effort — a metering
//! failure must never break the core graph operation.

use uuid::Uuid;

use crate::llm::TokenUsage;
use crate::redis_store::Redis;

pub struct UsageRecord {
    pub user_id: Uuid,
    pub request_id: Option<Uuid>,
    pub endpoint: &'static str,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    pub latency_ms: Option<i64>,
    pub status_code: i32,
}

/// Persist a usage record (failure-isolated: callers ignore errors).
pub async fn record_usage(pool: &sqlx::PgPool, record: &UsageRecord) -> Result<(), sqlx::Error> {
    let usage = record.usage;
    sqlx::query(
        r#"
        INSERT INTO usage_events
            (user_id, request_id, endpoint, provider, model,
             input_tokens, output_tokens, total_tokens, latency_ms, status_code)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(record.user_id)
    .bind(record.request_id)
    .bind(record.endpoint)
    .bind(&record.provider)
    .bind(&record.model)
    .bind(usage.map(|u| u.input_tokens))
    .bind(usage.map(|u| u.output_tokens))
    .bind(usage.map(|u| u.total_tokens))
    .bind(record.latency_ms)
    .bind(record.status_code)
    .execute(pool)
    .await?;
    Ok(())
}

/// Feed token usage into the Redis day/month counters so the quota check can
/// block future requests. Best-effort.
pub async fn add_token_usage(redis: &Redis, user_id: Uuid, usage: &TokenUsage) {
    if usage.total_tokens <= 0 {
        return;
    }
    for window in ["day", "month"] {
        if let Err(e) =
            crate::ratelimit::add_token_usage(redis, user_id, window, usage.total_tokens).await
        {
            tracing::warn!(error = %e, "failed to record token usage");
        }
    }
}

/// One-shot best-effort metering used by graph handlers: persist the usage
/// row and update token counters without ever failing the response.
pub async fn record_and_count(pool: &sqlx::PgPool, redis: &Redis, record: UsageRecord) {
    if let Err(e) = record_usage(pool, &record).await {
        tracing::warn!(error = %e, endpoint = record.endpoint, "usage recording failed");
    }
    if let Some(usage) = record.usage {
        add_token_usage(redis, record.user_id, &usage).await;
    }
}

/// Latency helper exposed for handlers.
pub fn now_ms(start: std::time::Instant) -> i64 {
    start.elapsed().as_millis().try_into().unwrap_or(0)
}
