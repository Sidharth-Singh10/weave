//! Audit log writer.
//!
//! Every privileged administrative change writes an audit entry, ideally in
//! the same transaction as the change itself (see the `write` executor API).
//! Secrets, OAuth tokens, and raw graph contents are never logged.

use serde_json::Value;
use sqlx::{Executor, Row};
use uuid::Uuid;

/// Action names shared across admin handlers (keep in sync with §26).
pub mod action {
    pub const USER_ROLE_CHANGED: &str = "user.role_changed";
    pub const USER_DISABLED: &str = "user.disabled";
    pub const USER_ENABLED: &str = "user.enabled";
    pub const USER_UPDATED: &str = "user.updated";
    pub const ROLE_CREATED: &str = "role.created";
    pub const ROLE_UPDATED: &str = "role.updated";
    pub const ROLE_DELETED: &str = "role.deleted";
    pub const ROLE_PERMISSIONS_CHANGED: &str = "role.permissions_changed";
    pub const POLICY_ROLE_UPDATED: &str = "policy.role_updated";
    pub const POLICY_USER_OVERRIDE_UPDATED: &str = "policy.user_override_updated";
    pub const POLICY_USER_OVERRIDE_REMOVED: &str = "policy.user_override_removed";
}

pub struct AuditEntry<'a> {
    pub actor_user_id: Uuid,
    pub action: &'a str,
    pub target_type: Option<&'a str>,
    pub target_id: Option<Uuid>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub ip_hash: Option<&'a str>,
}

/// Insert an audit entry through any executor (a pool or an open transaction).
pub async fn write<'a, E>(executor: E, entry: AuditEntry<'_>) -> Result<(), sqlx::Error>
where
    E: Executor<'a, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO audit_logs
            (actor_user_id, action, target_type, target_id, old_value, new_value, ip_hash)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(entry.actor_user_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id)
    .bind(entry.old_value)
    .bind(entry.new_value)
    .bind(entry.ip_hash)
    .execute(executor)
    .await?;
    Ok(())
}

/// A cursor page of audit entries.
pub struct AuditPage {
    pub entries: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}

/// Fetch audit entries (newest first) with cursor pagination and filters.
#[allow(clippy::too_many_arguments)]
pub async fn list(
    pool: &sqlx::PgPool,
    limit: i64,
    cursor: Option<&str>,
    actor: Option<&str>,
    action: Option<&str>,
    target_type: Option<&str>,
    since: Option<chrono::DateTime<chrono::Utc>>,
    until: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<AuditPage, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    // Cursor encodes the created_at of the previous page's last row.
    let cursor_ts: Option<chrono::DateTime<chrono::Utc>> = cursor
        .and_then(|c| chrono::DateTime::parse_from_rfc3339(c).ok())
        .map(|c| c.with_timezone(&chrono::Utc));

    let rows = sqlx::query(
        r#"
        SELECT a.id, a.action, a.target_type, a.target_id,
               a.old_value, a.new_value, a.created_at,
               u.email AS actor_email
        FROM audit_logs a
        LEFT JOIN users u ON u.id = a.actor_user_id
        WHERE ($1::timestamptz IS NULL OR a.created_at < $1)
          AND ($2::text IS NULL OR a.action = $2)
          AND ($3::text IS NULL OR a.target_type = $3)
          AND ($4::timestamptz IS NULL OR a.created_at >= $4)
          AND ($5::timestamptz IS NULL OR a.created_at <= $5)
          AND ($6::text IS NULL OR u.email ILIKE '%' || $6 || '%')
        ORDER BY a.created_at DESC, a.id DESC
        LIMIT $7
        "#,
    )
    .bind(cursor_ts)
    .bind(action)
    .bind(target_type)
    .bind(since)
    .bind(until)
    .bind(actor)
    .bind(limit + 1)
    .fetch_all(pool)
    .await?;

    let has_more = rows.len() as i64 > limit;
    let rows = &rows[..limit.min(rows.len() as i64) as usize];

    let entries: Vec<Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.get::<Uuid, _>("id"),
                "action": row.get::<String, _>("action"),
                "actor_email": row.get::<Option<String>, _>("actor_email"),
                "target_type": row.get::<Option<String>, _>("target_type"),
                "target_id": row.get::<Option<Uuid>, _>("target_id"),
                "old_value": row.get::<Option<Value>, _>("old_value"),
                "new_value": row.get::<Option<Value>, _>("new_value"),
                "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            })
        })
        .collect();

    let next_cursor = if has_more {
        rows.last().map(|row| {
            row.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .to_rfc3339()
        })
    } else {
        None
    };

    Ok(AuditPage {
        entries,
        next_cursor,
    })
}
