//! Redis access layer.
//!
//! Redis holds hot-path counters only (rate limits, token quotas, concurrency,
//! login-abuse throttles, OAuth state). PostgreSQL remains the source of truth.
//! If Redis is unavailable the application must still start; rate limiting
//! fails closed on expensive LLM endpoints (see the rate-limit module).

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
    pub fn connection(&self) -> ConnectionManager {
        self.mgr.clone()
    }

    /// Cheap liveness probe used by `/health/ready`.
    pub async fn ping(&self) -> redis::RedisResult<()> {
        redis::cmd("PING").query_async(&mut self.mgr.clone()).await
    }

    /// `SET key value EX ttl` — used for short-lived OAuth state.
    pub async fn set_ex(&self, key: &str, value: &str, ttl_seconds: u64) -> redis::RedisResult<()> {
        let mut conn = self.connection();
        redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
    }

    /// Atomic `GETDEL` — one-time consumption of OAuth state.
    pub async fn getdel(&self, key: &str) -> redis::RedisResult<Option<String>> {
        let mut conn = self.connection();
        redis::cmd("GETDEL").arg(key).query_async(&mut conn).await
    }

    /// Atomic `INCR` that sets `EXPIRE` on the first increment. Returns the
    /// current count for the window. Used for login-abuse throttles. Callers
    /// compare the count against a configured limit; the returned value is
    /// approximate under high concurrency, which is acceptable for
    /// short-lived abuse throttling (application rate limits use a strict
    /// Lua decision script in the rate-limit module).
    pub async fn increment_window(
        &self,
        key: &str,
        window_seconds: u64,
    ) -> redis::RedisResult<u64> {
        let script = redis::Script::new(
            r#"
            local current = redis.call('INCR', KEYS[1])
            if current == 1 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            return current
            "#,
        );
        let mut conn = self.connection();
        let count: i64 = script
            .key(key)
            .arg(window_seconds)
            .invoke_async(&mut conn)
            .await?;
        Ok(count.max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    // Integration-style tests against a live Redis (gated by REDIS_URL).
    use super::*;

    #[tokio::test]
    async fn increment_window_counts_and_expires() {
        let url = std::env::var("REDIS_URL").ok();
        let Some(url) = url else {
            eprintln!("skipping: REDIS_URL not set");
            return;
        };
        let Ok(redis) = Redis::connect(&url).await else {
            eprintln!("skipping: redis unavailable");
            return;
        };
        let key = format!("test:window:{}", uuid::Uuid::new_v4());
        assert_eq!(redis.increment_window(&key, 5).await.unwrap(), 1);
        assert_eq!(redis.increment_window(&key, 5).await.unwrap(), 2);
        assert_eq!(redis.increment_window(&key, 5).await.unwrap(), 3);

        // TTL should be ~5s.
        let mut conn = redis.connection();
        let ttl: i64 = redis::cmd("TTL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert!(ttl > 0 && ttl <= 5);
    }

    #[tokio::test]
    async fn getdel_consumes_once() {
        let url = std::env::var("REDIS_URL").ok();
        let Some(url) = url else {
            eprintln!("skipping: REDIS_URL not set");
            return;
        };
        let Ok(redis) = Redis::connect(&url).await else {
            eprintln!("skipping: redis unavailable");
            return;
        };
        let key = format!("test:getdel:{}", uuid::Uuid::new_v4());
        redis.set_ex(&key, "nonce-1", 30).await.unwrap();
        assert_eq!(
            redis.getdel(&key).await.unwrap().as_deref(),
            Some("nonce-1")
        );
        assert_eq!(redis.getdel(&key).await.unwrap(), None);
    }
}
