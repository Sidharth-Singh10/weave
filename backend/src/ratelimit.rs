//! Redis-backed rate limiting and quota enforcement.
//!
//! Semantics (documented in the admin UI and README):
//! - Requests/min (and hour/day): completed-or-accepted requests in a fixed
//!   window. A blocked request does not consume quota.
//! - Tokens/day (and month): total provider-reported tokens consumed.
//! - Concurrent requests: requests currently executing.
//!
//! Effective limits resolve global → role → user override (more specific
//! wins) and are clamped to the configured global hard ceiling. All counter
//! decisions are atomic Lua scripts (no non-atomic GET/calculate/SET).
//!
//! Redis failure policy: authentication relies on PostgreSQL (unaffected).
//! For expensive graph/LLM endpoints the middleware **fails closed** (503)
//! rather than silently allowing unlimited LLM traffic. Cheap endpoints do
//! not use this module.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::auth::middleware::UserContext;
use crate::config::Config;
use crate::policy::{Limits, RawRule};
use crate::redis_store::Redis;

// ---------------------------------------------------------------------------
// Policy resolution
// ---------------------------------------------------------------------------

/// Load the limits of one policy scope/endpoint combination.
async fn load_policy_limits(
    pool: &PgPool,
    scope: &str,
    role_id: Option<Uuid>,
    user_id: Option<Uuid>,
    endpoint: Option<&str>,
) -> Result<Limits, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT rr.metric, rr.time_window, rr.limit_value
        FROM rate_limit_policies p
        JOIN rate_limit_rules rr ON rr.policy_id = p.id
        WHERE p.scope_type = $1
          AND ($2::uuid IS NULL OR p.role_id = $2)
          AND ($3::uuid IS NULL OR p.user_id = $3)
          AND ($4::text IS NULL OR p.endpoint = $4)
        ORDER BY rr.metric, rr.time_window
        "#,
    )
    .bind(scope)
    .bind(role_id)
    .bind(user_id)
    .bind(endpoint)
    .fetch_all(pool)
    .await?;

    let rules: Vec<RawRule> = rows
        .iter()
        .filter_map(|r| {
            let metric: Option<String> = r.get("metric");
            metric.map(|metric| RawRule {
                metric,
                time_window: r.get("time_window"),
                limit: r.get("limit_value"),
            })
        })
        .collect();
    Ok(Limits::from_rules(&rules))
}

fn merge_first_non_none(merged: &mut Limits, levels: &[Limits]) {
    for level in levels {
        macro_rules! take {
            ($field:ident) => {
                if merged.$field.is_none() {
                    merged.$field = level.$field;
                }
            };
        }
        take!(requests_per_minute);
        take!(requests_per_hour);
        take!(requests_per_day);
        take!(tokens_per_minute);
        take!(tokens_per_day);
        take!(tokens_per_month);
        take!(concurrent_requests);
    }
}

fn apply_ceilings(limits: &mut Limits, config: &Config) {
    if let Some(v) = limits.requests_per_minute {
        limits.requests_per_minute = Some(v.min(config.hard_ceiling_requests_per_minute as i64));
    }
    if let Some(v) = limits.tokens_per_day {
        limits.tokens_per_day = Some(v.min(config.hard_ceiling_tokens_per_day as i64));
    }
}

/// Resolve the effective limits for a user on an endpoint.
///
/// Precedence (most specific wins): user endpoint → user generic → role
/// endpoint → role generic → global endpoint → global generic. Then clamp to
/// the global hard ceiling.
pub async fn resolve_limits(
    pool: &PgPool,
    config: &Config,
    user: &UserContext,
    endpoint: &str,
) -> Result<Limits, sqlx::Error> {
    let levels = [
        load_policy_limits(pool, "user", None, Some(user.user_id), Some(endpoint)).await?,
        load_policy_limits(pool, "user", None, Some(user.user_id), None).await?,
        load_policy_limits(pool, "role", Some(user.role_id), None, Some(endpoint)).await?,
        load_policy_limits(pool, "role", Some(user.role_id), None, None).await?,
        load_policy_limits(pool, "global", None, None, Some(endpoint)).await?,
        load_policy_limits(pool, "global", None, None, None).await?,
    ];

    let mut merged = Limits::default();
    merge_first_non_none(&mut merged, &levels);
    apply_ceilings(&mut merged, config);
    Ok(merged)
}

// ---------------------------------------------------------------------------
// Redis counter helpers (atomic Lua)
// ---------------------------------------------------------------------------

const WINDOW_SECONDS: &[(&str, u64)] = &[
    ("minute", 60),
    ("hour", 3600),
    ("day", 86400),
    ("month", 2_592_000),
];

fn window_seconds(window: &str) -> u64 {
    WINDOW_SECONDS
        .iter()
        .find(|(w, _)| *w == window)
        .map(|(_, s)| *s)
        .unwrap_or(60)
}

fn request_key(user_id: Uuid, endpoint: &str, metric: &str, window: &str) -> String {
    format!("rl:{user_id}:{endpoint}:{metric}:{window}")
}

fn token_key(user_id: Uuid, metric: &str, window: &str) -> String {
    format!("tok:{user_id}:{metric}:{window}")
}

fn concurrency_key(user_id: Uuid, endpoint: &str) -> String {
    format!("rlc:{user_id}:{endpoint}")
}

/// Atomic check-and-increment for a single (metric, window) counter.
/// Returns (allowed, remaining, ttl_ms).
const CHECK_INCR_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[2])
end
local ttl_ms = redis.call('PTTL', KEYS[1])
if current > tonumber(ARGV[1]) then
    redis.call('DECR', KEYS[1])
    return {0, 0, ttl_ms}
end
return {1, tonumber(ARGV[1]) - current, ttl_ms}
"#;

async fn check_and_increment(
    redis: &Redis,
    key: &str,
    limit: i64,
    window_secs: u64,
) -> redis::RedisResult<(bool, i64, u64)> {
    let script = redis::Script::new(CHECK_INCR_SCRIPT);
    let mut conn = redis.connection();
    let (allowed, remaining, ttl_ms): (i64, i64, i64) = script
        .key(key)
        .arg(limit)
        .arg(window_secs)
        .invoke_async(&mut conn)
        .await?;
    Ok((allowed == 1, remaining.max(0), ttl_ms.max(0) as u64))
}

/// Atomic check (no increment) whether a token counter already exceeds its
/// limit. Returns true when the quota is exhausted.
const CHECK_TOKEN_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
if current >= tonumber(ARGV[1]) then
    return {0}
end
return {1}
"#;

async fn check_token_quota(redis: &Redis, key: &str, limit: i64) -> redis::RedisResult<bool> {
    let script = redis::Script::new(CHECK_TOKEN_SCRIPT);
    let mut conn = redis.connection();
    let (allowed,): (i64,) = script.key(key).arg(limit).invoke_async(&mut conn).await?;
    Ok(allowed == 0)
}

/// Atomically add `usage` tokens to a counter (called after an LLM call).
const ADD_TOKEN_SCRIPT: &str = r#"
local current = redis.call('INCRBY', KEYS[1], ARGV[1])
if current == ARGV[1] then
    redis.call('EXPIRE', KEYS[1], ARGV[2])
end
return current
"#;

// Wired into LLM handlers by usage metering (Phase 5).
#[allow(dead_code)]
pub async fn add_token_usage(
    redis: &Redis,
    user_id: Uuid,
    window: &str,
    usage: i64,
) -> redis::RedisResult<i64> {
    let script = redis::Script::new(ADD_TOKEN_SCRIPT);
    let mut conn = redis.connection();
    let key = token_key(user_id, "tokens", window);
    script
        .key(key)
        .arg(usage)
        .arg(window_seconds(window))
        .invoke_async(&mut conn)
        .await
}

// ---------------------------------------------------------------------------
// Request / token / concurrency enforcement
// ---------------------------------------------------------------------------

/// Check all request-window limits for `resolved`. Returns `Some(retry_after)`
/// when the request is blocked.
pub async fn check_request_limits(
    redis: &Redis,
    user_id: Uuid,
    endpoint: &str,
    resolved: &Limits,
) -> Result<Option<u64>, redis::RedisError> {
    let checks: Vec<(&Option<i64>, &str)> = vec![
        (&resolved.requests_per_minute, "minute"),
        (&resolved.requests_per_hour, "hour"),
        (&resolved.requests_per_day, "day"),
    ];
    for (limit, window) in checks {
        if let Some(limit) = limit {
            let key = request_key(user_id, endpoint, "requests", window);
            let (allowed, _remaining, ttl_ms) =
                check_and_increment(redis, &key, *limit, window_seconds(window)).await?;
            if !allowed {
                let retry = if ttl_ms > 0 {
                    ttl_ms.div_ceil(1000).max(1)
                } else {
                    window_seconds(window)
                };
                return Ok(Some(retry));
            }
        }
    }
    Ok(None)
}

/// Check the user's token quota for the day/month windows. Returns true when
/// any applicable quota is exhausted.
pub async fn check_token_limits(
    redis: &Redis,
    user_id: Uuid,
    resolved: &Limits,
) -> Result<bool, redis::RedisError> {
    for (limit, window) in [
        (&resolved.tokens_per_day, "day"),
        (&resolved.tokens_per_month, "month"),
    ] {
        if let Some(limit) = limit {
            if check_token_quota(redis, &token_key(user_id, "tokens", window), *limit).await? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// RAII guard releasing a concurrency slot on drop (even on panic/error).
pub struct ConcurrencyGuard {
    redis: Redis,
    key: String,
}

impl Drop for ConcurrencyGuard {
    fn drop(&mut self) {
        let mut conn = self.redis.connection();
        let key = self.key.clone();
        tokio::spawn(async move {
            let _ = redis::cmd("DECR")
                .arg(&key)
                .query_async::<()>(&mut conn)
                .await;
        });
    }
}

/// Result of a concurrency-slot acquisition.
pub enum ConcurrencyResult {
    /// A slot was acquired; dropping the guard releases it.
    Acquired(ConcurrencyGuard),
    /// A concurrency limit is configured and the user is at capacity.
    Limited,
    /// No concurrency limit configured; nothing to acquire.
    NotConfigured,
}

/// Acquire a concurrency slot if `resolved.concurrent_requests` allows it.
pub async fn acquire_concurrency(
    redis: &Redis,
    user_id: Uuid,
    endpoint: &str,
    resolved: &Limits,
) -> Result<ConcurrencyResult, redis::RedisError> {
    let Some(limit) = resolved.concurrent_requests else {
        return Ok(ConcurrencyResult::NotConfigured);
    };
    if limit <= 0 {
        return Ok(ConcurrencyResult::NotConfigured);
    }
    let key = concurrency_key(user_id, endpoint);
    let script = redis::Script::new(
        r#"
        local current = redis.call('INCR', KEYS[1])
        if current == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[2])
        end
        if current > tonumber(ARGV[1]) then
            redis.call('DECR', KEYS[1])
            return {0}
        end
        return {1}
        "#,
    );
    let mut conn = redis.connection();
    let (ok,): (i64,) = script
        .key(&key)
        .arg(limit)
        .arg(300)
        .invoke_async(&mut conn)
        .await?;
    if ok == 1 {
        Ok(ConcurrencyResult::Acquired(ConcurrencyGuard {
            redis: redis.clone(),
            key,
        }))
    } else {
        Ok(ConcurrencyResult::Limited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn window_seconds_map() {
        assert_eq!(window_seconds("minute"), 60);
        assert_eq!(window_seconds("hour"), 3600);
        assert_eq!(window_seconds("day"), 86400);
        assert_eq!(window_seconds("month"), 2_592_000);
        assert_eq!(window_seconds("unknown"), 60);
    }

    #[test]
    fn merge_prefers_specific_levels() {
        let mut merged = Limits::default();
        let levels = [
            Limits {
                requests_per_minute: Some(120),
                ..Limits::default()
            },
            Limits {
                requests_per_minute: Some(60),
                tokens_per_day: Some(2_000_000),
                ..Limits::default()
            },
            Limits {
                tokens_per_day: Some(500_000),
                ..Limits::default()
            },
        ];
        merge_first_non_none(&mut merged, &levels);
        assert_eq!(merged.requests_per_minute, Some(120)); // user override wins
        assert_eq!(merged.tokens_per_day, Some(2_000_000)); // role generic wins
        assert_eq!(merged.concurrent_requests, None); // no level sets it
    }

    #[test]
    fn hard_ceiling_clamps() {
        let mut config = Config::test_default();
        config.hard_ceiling_requests_per_minute = 100;
        config.hard_ceiling_tokens_per_day = 10_000_000;
        let mut limits = Limits {
            requests_per_minute: Some(1000),
            tokens_per_day: Some(20_000_000),
            ..Limits::default()
        };
        apply_ceilings(&mut limits, &config);
        assert_eq!(limits.requests_per_minute, Some(100));
        assert_eq!(limits.tokens_per_day, Some(10_000_000));
    }
}
