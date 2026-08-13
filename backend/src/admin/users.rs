//! Admin user management routes.

use axum::extract::{Path, Query, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::audit;
use crate::auth::middleware::{UserContext, require_permission};
use crate::error::{ApiError, ApiErrorKind};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(list_users))
        .route("/api/admin/users/{id}", get(get_user))
        .route("/api/admin/users/{id}", patch(update_user))
}

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    pub search: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    20
}

async fn list_users(
    State(state): State<AppState>,
    user: UserContext,
    Query(q): Query<UserQuery>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.users.read")?;
    let page = q.page.max(1);
    let page_size = q.page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;

    let search = q
        .search
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let role = q.role.as_deref().filter(|s| !s.is_empty());
    let status = q.status.as_deref().filter(|s| !s.is_empty());

    let rows = sqlx::query(
        r#"
        SELECT u.id, u.email, u.name, u.avatar_url, u.role_id,
               r.name AS role_name, u.status, u.created_at, u.last_login_at
        FROM users u
        JOIN roles r ON r.id = u.role_id
        WHERE ($1::text IS NULL OR u.email ILIKE '%' || $1 || '%'
            OR u.name ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL OR r.name = $2)
          AND ($3::text IS NULL OR u.status = $3)
        ORDER BY u.created_at DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(search)
    .bind(role)
    .bind(status)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT count(*) FROM users u
        JOIN roles r ON r.id = u.role_id
        WHERE ($1::text IS NULL OR u.email ILIKE '%' || $1 || '%'
            OR u.name ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL OR r.name = $2)
          AND ($3::text IS NULL OR u.status = $3)
        "#,
    )
    .bind(search)
    .bind(role)
    .bind(status)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)?;

    let items: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.get::<Uuid, _>("id"),
                "email": row.get::<String, _>("email"),
                "name": row.get::<Option<String>, _>("name"),
                "avatar_url": row.get::<Option<String>, _>("avatar_url"),
                "role_id": row.get::<Uuid, _>("role_id"),
                "role": row.get::<String, _>("role_name"),
                "status": row.get::<String, _>("status"),
                "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                "last_login_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_login_at"),
            })
        })
        .collect();

    Ok(Json(json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

async fn get_user(
    State(state): State<AppState>,
    user: UserContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.users.read")?;
    let row = sqlx::query(
        r#"
        SELECT u.id, u.email, u.name, u.avatar_url, u.role_id,
               r.name AS role_name, u.status, u.created_at, u.updated_at, u.last_login_at
        FROM users u
        JOIN roles r ON r.id = u.role_id
        WHERE u.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::new(ApiErrorKind::NotFound))?;

    Ok(Json(json!({
        "id": row.get::<Uuid, _>("id"),
        "email": row.get::<String, _>("email"),
        "name": row.get::<Option<String>, _>("name"),
        "avatar_url": row.get::<Option<String>, _>("avatar_url"),
        "role_id": row.get::<Uuid, _>("role_id"),
        "role": row.get::<String, _>("role_name"),
        "status": row.get::<String, _>("status"),
        "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "last_login_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_login_at"),
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role_id: Option<Uuid>,
    pub status: Option<String>,
    pub name: Option<String>,
}

async fn update_user(
    State(state): State<AppState>,
    actor: UserContext,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.users.update")?;

    if id == actor.user_id {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "use the account flow to modify your own profile".into(),
        )));
    }

    let (email, current_role_id, current_role, current_status) = sqlx::query_as::<_, (String, Uuid, String, String)>(
        "SELECT u.email, u.role_id, r.name, u.status FROM users u JOIN roles r ON r.id = u.role_id WHERE u.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::new(ApiErrorKind::NotFound))?;

    // --- Role change with final-admin protection -------------------------
    if let Some(new_role_id) = req.role_id {
        if new_role_id != current_role_id {
            let (new_role_name,): (String,) =
                sqlx::query_as("SELECT name FROM roles WHERE id = $1")
                    .bind(new_role_id)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(ApiError::from)?
                    .ok_or_else(|| {
                        ApiError::new(ApiErrorKind::InvalidRequest("unknown role".into()))
                    })?;

            let is_last_admin = current_role == "admin"
                && count_active_admins(&state.db)
                    .await
                    .map_err(ApiError::from)?
                    <= 1;
            if is_last_admin && new_role_name != "admin" {
                return Err(ApiError::new(ApiErrorKind::Conflict(
                    "cannot remove admin from the last active admin".into(),
                )));
            }

            let mut tx = state.db.begin().await.map_err(ApiError::from)?;
            sqlx::query("UPDATE users SET role_id = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(new_role_id)
                .execute(&mut *tx)
                .await
                .map_err(ApiError::from)?;
            audit::write(
                &mut *tx,
                audit::AuditEntry {
                    actor_user_id: actor.user_id,
                    action: audit::action::USER_ROLE_CHANGED,
                    target_type: Some("user"),
                    target_id: Some(id),
                    old_value: Some(json!({ "role_id": current_role_id })),
                    new_value: Some(json!({ "role_id": new_role_id })),
                    ip_hash: Some(&actor.ip_hash),
                },
            )
            .await
            .map_err(ApiError::from)?;
            tx.commit().await.map_err(ApiError::from)?;

            // Role change alters the user's effective policy.
            invalidate_user_policy(&state, id).await;
        }
    }

    // --- Status change with final-admin protection -----------------------
    if let Some(status) = req.status.as_deref() {
        if !matches!(status, "active" | "disabled" | "suspended") {
            return Err(ApiError::new(ApiErrorKind::InvalidRequest(
                "status must be active, disabled, or suspended".into(),
            )));
        }
        if status != current_status {
            let disabling = status != "active";
            if disabling
                && current_status == "active"
                && current_role == "admin"
                && count_active_admins(&state.db)
                    .await
                    .map_err(ApiError::from)?
                    <= 1
            {
                return Err(ApiError::new(ApiErrorKind::Conflict(
                    "cannot disable the last active admin".into(),
                )));
            }
            let mut tx = state.db.begin().await.map_err(ApiError::from)?;
            sqlx::query("UPDATE users SET status = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(status)
                .execute(&mut *tx)
                .await
                .map_err(ApiError::from)?;
            let action = if status == "active" {
                audit::action::USER_ENABLED
            } else if status == "disabled" {
                audit::action::USER_DISABLED
            } else {
                audit::action::USER_UPDATED
            };
            audit::write(
                &mut *tx,
                audit::AuditEntry {
                    actor_user_id: actor.user_id,
                    action,
                    target_type: Some("user"),
                    target_id: Some(id),
                    old_value: Some(json!({ "status": current_status })),
                    new_value: Some(json!({ "status": status })),
                    ip_hash: Some(&actor.ip_hash),
                },
            )
            .await
            .map_err(ApiError::from)?;
            tx.commit().await.map_err(ApiError::from)?;
        }
    }

    // --- Name change ------------------------------------------------------
    if let Some(name) = req.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let mut tx = state.db.begin().await.map_err(ApiError::from)?;
        sqlx::query("UPDATE users SET name = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        audit::write(
            &mut *tx,
            audit::AuditEntry {
                actor_user_id: actor.user_id,
                action: audit::action::USER_UPDATED,
                target_type: Some("user"),
                target_id: Some(id),
                old_value: None,
                new_value: Some(json!({ "name": name })),
                ip_hash: Some(&actor.ip_hash),
            },
        )
        .await
        .map_err(ApiError::from)?;
        tx.commit().await.map_err(ApiError::from)?;
    }

    Ok(Json(json!({
        "ok": true,
        "user": { "id": id, "email": email, "role": current_role, "status": current_status }
    })))
}

async fn count_active_admins(pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT count(*) FROM users u
        JOIN roles r ON r.id = u.role_id
        WHERE r.name = 'admin' AND u.status = 'active'
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Delete the user-level policy cache keys after an override-affecting change.
pub async fn invalidate_user_policy(state: &AppState, user_id: Uuid) {
    let key = format!("weave:policy:user:{user_id}");
    let mut conn = state.redis.connection();
    let _ = redis::cmd("DEL")
        .arg(key)
        .query_async::<()>(&mut conn)
        .await;
}
