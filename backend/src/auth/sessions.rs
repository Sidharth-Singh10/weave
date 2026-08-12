//! Server-side session management.
//!
//! The browser holds an opaque random session token in an HttpOnly cookie;
//! PostgreSQL stores only a SHA-256 hash of that token. Never store the raw
//! token in the database and never log it.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// A session row joined with its user and role.
#[derive(Debug, Clone, FromRow)]
pub struct SessionUser {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub role_id: Uuid,
    pub role_name: String,
    pub status: String,
}

/// 32 random bytes rendered as 64 hex chars (256 bits of entropy).
pub fn generate_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// SHA-256 hex digest of the raw token — the value stored in PostgreSQL.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Insert a session row for `token`. Returns the new session id.
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token: &str,
    ttl_seconds: i64,
    ip_hash: Option<&str>,
    user_agent: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let token_hash = hash_token(token);
    let expires_at = Utc::now() + chrono::Duration::seconds(ttl_seconds);
    let row = sqlx::query_as::<_, (Uuid,)>(
        r#"
        INSERT INTO sessions (user_id, session_token_hash, expires_at, ip_hash, user_agent)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .bind(ip_hash)
    .bind(user_agent)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Look up a session by raw token. Returns `None` when no matching row exists.
pub async fn lookup_session(
    pool: &PgPool,
    token: &str,
) -> Result<Option<SessionUser>, sqlx::Error> {
    let token_hash = hash_token(token);
    fetch_user(pool, token_hash).await
}

async fn fetch_user(pool: &PgPool, token_hash: String) -> Result<Option<SessionUser>, sqlx::Error> {
    let row = sqlx::query_as::<_, SessionUser>(
        r#"
        SELECT s.id AS session_id,
               s.user_id,
               s.expires_at,
               s.revoked_at,
               u.email,
               u.name,
               u.avatar_url,
               u.role_id,
               r.name AS role_name,
               u.status
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        JOIN roles r ON r.id = u.role_id
        WHERE s.session_token_hash = $1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Revoke a single session. Returns true when a live session was revoked.
pub async fn revoke_session(pool: &PgPool, token: &str) -> Result<bool, sqlx::Error> {
    let token_hash = hash_token(token);
    let res = sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE session_token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(token_hash)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Revoke every live session for a user.
pub async fn revoke_all_sessions(pool: &PgPool, user_id: Uuid) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Best-effort heartbeat; a failure here must never break a request.
pub async fn touch_session(pool: &PgPool, session_id: Uuid) {
    let _ = sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_hex_chars_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_is_stable_and_irreversible_by_design() {
        let t = generate_token();
        assert_eq!(hash_token(&t), hash_token(&t));
        assert_ne!(hash_token(&t), hash_token(&format!("{t}x")));
        assert_eq!(hash_token(&t).len(), 64);
        // The raw token must never equal its hash.
        assert_ne!(hash_token(&t), t);
    }
}
