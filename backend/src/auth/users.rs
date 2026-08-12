//! User records: lookup, creation, role resolution, and bootstrap admin.

use std::collections::HashSet;

use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::oauth::OidcIdentity;
use crate::config::Config;

#[derive(Debug, Clone, FromRow)]
// `google_subject`/`role_id` are exposed by admin routes in later phases.
#[allow(dead_code)]
pub struct User {
    pub id: Uuid,
    pub google_subject: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub role_id: Uuid,
    pub role_name: String,
    pub status: String,
}

/// Look up a user by stable Google subject.
pub async fn get_by_subject(pool: &PgPool, subject: &str) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, User>(
        r#"
        SELECT u.id, u.google_subject, u.email, u.name, u.avatar_url,
               u.role_id, r.name AS role_name, u.status
        FROM users u
        JOIN roles r ON r.id = u.role_id
        WHERE u.google_subject = $1
        "#,
    )
    .bind(subject)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Find an existing user or create one on first login.
///
/// Bootstrap admin: when the authenticated email is listed in
/// `BOOTSTRAP_ADMIN_EMAILS` and no active admin exists yet, the account is
/// created with the admin role. After that, admin privileges come only from
/// the database.
pub async fn find_or_create_user(
    pool: &PgPool,
    config: &Config,
    identity: &OidcIdentity,
) -> Result<(User, bool), sqlx::Error> {
    if let Some(user) = get_by_subject(pool, &identity.subject).await? {
        refresh_profile(pool, &user.id, identity).await;
        touch_last_login(pool, &user.id).await;
        let refreshed = get_by_subject(pool, &identity.subject).await?;
        return Ok((refreshed.unwrap_or(user), false));
    }

    let role_id = if is_bootstrap_admin(&config.bootstrap_admin_emails, &identity.email)
        && !admin_exists(pool).await?
    {
        role_id_by_name(pool, "admin").await?
    } else {
        role_id_by_name(pool, "member").await?
    };

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (google_subject, email, name, avatar_url, role_id, status, last_login_at)
        VALUES ($1, $2, $3, $4, $5, 'active', now())
        RETURNING id, google_subject, email, name, avatar_url, role_id,
                  (SELECT name FROM roles WHERE id = users.role_id) AS role_name, status
        "#,
    )
    .bind(&identity.subject)
    .bind(&identity.email)
    .bind(&identity.name)
    .bind(&identity.picture)
    .bind(role_id)
    .fetch_one(pool)
    .await?;

    tracing::info!(
        email = %user.email,
        role = %user.role_name,
        "user created"
    );
    Ok((user, true))
}

/// Permissions granted to a role.
pub async fn load_permissions(
    pool: &PgPool,
    role_id: Uuid,
) -> Result<HashSet<String>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT p.key
        FROM role_permissions rp
        JOIN permissions p ON p.id = rp.permission_id
        WHERE rp.role_id = $1
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

async fn refresh_profile(pool: &PgPool, user_id: &Uuid, identity: &OidcIdentity) {
    let _ = sqlx::query("UPDATE users SET name = $2, avatar_url = $3 WHERE id = $1")
        .bind(user_id)
        .bind(&identity.name)
        .bind(&identity.picture)
        .execute(pool)
        .await;
}

async fn touch_last_login(pool: &PgPool, user_id: &Uuid) {
    let _ = sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

fn is_bootstrap_admin(bootstrap_emails: &[String], email: &str) -> bool {
    bootstrap_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
}

async fn admin_exists(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) = sqlx::query_as(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM users u
            JOIN roles r ON r.id = u.role_id
            WHERE r.name = 'admin' AND u.status = 'active'
        )
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn role_id_by_name(pool: &PgPool, name: &str) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as("SELECT id FROM roles WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_admin_is_case_insensitive() {
        let emails = vec!["Admin@Example.com".to_string()];
        assert!(is_bootstrap_admin(&emails, "admin@example.com"));
        assert!(!is_bootstrap_admin(&emails, "other@example.com"));
        assert!(!is_bootstrap_admin(&[], "admin@example.com"));
    }
}
