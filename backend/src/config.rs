//! Typed runtime configuration, loaded once from the environment at startup.
//!
//! All values are read from environment variables (see `.env.example`).
//! `DATABASE_URL` and `REDIS_URL` are required; the application refuses to
//! start without them so the API never silently runs without durable state.

#[derive(Clone, Debug)]
// Several fields are consumed by later phases (auth, rate limiting); they are
// parsed now so startup validation and config stay in one place.
#[allow(dead_code)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,

    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_uri: Option<String>,

    pub frontend_url: String,
    pub allowed_origins: Vec<String>,
    pub auth_stub: bool,

    pub session_cookie_name: String,
    pub session_ttl_seconds: i64,
    pub bootstrap_admin_emails: Vec<String>,

    /// Global default limits, applied when no policy row resolves.
    pub rate_limit_default_requests_per_minute: u64,
    pub rate_limit_default_tokens_per_day: u64,
    /// Absolute safety ceiling; effective limits are min(configured, ceiling).
    pub hard_ceiling_requests_per_minute: u64,
    pub hard_ceiling_tokens_per_day: u64,

    /// Login-abuse throttles (short-lived, per IP).
    pub oauth_init_limit_per_10_min: u64,
    pub oauth_callback_limit_per_10_min: u64,

    pub max_body_bytes: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = required("DATABASE_URL");
        let redis_url = required("REDIS_URL");
        let frontend_url = env("FRONTEND_URL", "http://localhost:3000");

        let mut allowed_origins = env_list("CORS_ALLOWED_ORIGINS");
        if !allowed_origins.iter().any(|o| o == &frontend_url) {
            allowed_origins.push(frontend_url.clone());
        }

        let rate_limit_default_requests_per_minute =
            env_u64("RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE", 30);
        let rate_limit_default_tokens_per_day =
            env_u64("RATE_LIMIT_DEFAULT_TOKENS_PER_DAY", 500000);

        Self {
            database_url,
            redis_url,
            google_client_id: env_opt("GOOGLE_CLIENT_ID"),
            google_client_secret: env_opt("GOOGLE_CLIENT_SECRET"),
            google_redirect_uri: env_opt("GOOGLE_REDIRECT_URI"),
            frontend_url,
            allowed_origins,
            auth_stub: env_bool("AUTH_STUB"),
            session_cookie_name: env("SESSION_COOKIE_NAME", "weave_session"),
            session_ttl_seconds: env_i64("SESSION_TTL_SECONDS", 2_592_000),
            bootstrap_admin_emails: env_list("BOOTSTRAP_ADMIN_EMAILS"),
            rate_limit_default_requests_per_minute,
            rate_limit_default_tokens_per_day,
            hard_ceiling_requests_per_minute: env_u64("HARD_CEILING_REQUESTS_PER_MINUTE", 300),
            hard_ceiling_tokens_per_day: env_u64("HARD_CEILING_TOKENS_PER_DAY", 20_000_000),
            oauth_init_limit_per_10_min: env_u64("OAUTH_INIT_LIMIT_PER_10_MIN", 20),
            oauth_callback_limit_per_10_min: env_u64("OAUTH_CALLBACK_LIMIT_PER_10_MIN", 10),
            max_body_bytes: env_usize("MAX_BODY_BYTES", 2 * 1024 * 1024),
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set (see backend/.env.example)"))
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_list(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests mutate process-wide env vars, so they must not run in parallel.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const CONFIG_KEYS: &[&str] = &[
        "DATABASE_URL",
        "REDIS_URL",
        "FRONTEND_URL",
        "CORS_ALLOWED_ORIGINS",
        "AUTH_STUB",
        "SESSION_COOKIE_NAME",
        "SESSION_TTL_SECONDS",
        "BOOTSTRAP_ADMIN_EMAILS",
        "RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE",
        "RATE_LIMIT_DEFAULT_TOKENS_PER_DAY",
        "HARD_CEILING_REQUESTS_PER_MINUTE",
        "HARD_CEILING_TOKENS_PER_DAY",
        "OAUTH_INIT_LIMIT_PER_10_MIN",
        "OAUTH_CALLBACK_LIMIT_PER_10_MIN",
        "MAX_BODY_BYTES",
        "GOOGLE_CLIENT_ID",
        "GOOGLE_CLIENT_SECRET",
        "GOOGLE_REDIRECT_URI",
    ];

    fn with_clean_env(entries: &[(&str, &str)]) {
        for key in CONFIG_KEYS {
            // SAFETY: test-only single-threaded env mutation (serialized by ENV_LOCK).
            unsafe { std::env::remove_var(key) };
        }
        for (k, v) in entries {
            // SAFETY: test-only single-threaded env mutation (serialized by ENV_LOCK).
            unsafe { std::env::set_var(k, v) };
        }
    }

    #[test]
    fn defaults_apply() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_clean_env(&[("DATABASE_URL", "postgres://t"), ("REDIS_URL", "redis://t")]);
        let c = Config::from_env();
        assert_eq!(c.session_cookie_name, "weave_session");
        assert_eq!(c.session_ttl_seconds, 2_592_000);
        assert_eq!(c.rate_limit_default_requests_per_minute, 30);
        assert_eq!(c.rate_limit_default_tokens_per_day, 500_000);
        assert_eq!(c.hard_ceiling_requests_per_minute, 300);
        assert_eq!(c.hard_ceiling_tokens_per_day, 20_000_000);
        assert!(!c.auth_stub);
        assert_eq!(c.max_body_bytes, 2 * 1024 * 1024);
        assert!(
            c.allowed_origins
                .contains(&"http://localhost:3000".to_string())
        );
    }

    #[test]
    fn overrides_and_lists_parse() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_clean_env(&[
            ("DATABASE_URL", "postgres://t"),
            ("REDIS_URL", "redis://t"),
            ("AUTH_STUB", "true"),
            ("SESSION_TTL_SECONDS", "60"),
            ("HARD_CEILING_REQUESTS_PER_MINUTE", "100"),
            ("BOOTSTRAP_ADMIN_EMAILS", "a@x.com, b@x.com"),
            ("CORS_ALLOWED_ORIGINS", "https://a.example.com"),
        ]);
        let c = Config::from_env();
        assert!(c.auth_stub);
        assert_eq!(c.session_ttl_seconds, 60);
        assert_eq!(c.hard_ceiling_requests_per_minute, 100);
        assert_eq!(c.bootstrap_admin_emails, vec!["a@x.com", "b@x.com"]);
        assert!(
            c.allowed_origins
                .contains(&"https://a.example.com".to_string())
        );
        assert!(
            c.allowed_origins
                .contains(&"http://localhost:3000".to_string())
        );
    }
}
