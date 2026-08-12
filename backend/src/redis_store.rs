//! Redis access layer.
//!
//! Redis holds hot-path counters only (rate limits, token quotas, concurrency,
//! login-abuse throttles). PostgreSQL remains the source of truth. If Redis is
//! unavailable the application must still start; rate limiting fails closed on
//! expensive LLM endpoints (see the rate-limit module).

use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct Redis {
    mgr: ConnectionManager,
}

impl Redis {
    pub async fn connect(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let mgr = ConnectionManager::new(client).await?;
        tracing::info!("redis connected");
        Ok(Self { mgr })
    }

    /// Clone of the auto-reconnecting connection handle.
    // Used by rate-limit/auth-abuse counters (Phase 4).
    #[allow(dead_code)]
    pub fn connection(&self) -> ConnectionManager {
        self.mgr.clone()
    }

    /// Cheap liveness probe used by `/health/ready`.
    pub async fn ping(&self) -> redis::RedisResult<()> {
        redis::cmd("PING").query_async(&mut self.mgr.clone()).await
    }
}
