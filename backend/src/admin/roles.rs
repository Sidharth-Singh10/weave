//! Admin role management routes.

use axum::extract::{Path, State};
use axum::routing::{delete, get, patch, post};
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
        .route("/api/admin/roles", get(list_roles))
        .route("/api/admin/roles", post(create_role))
        .route("/api/admin/roles/{id}", patch(update_role))
        .route("/api/admin/roles/{id}", delete(delete_role))
}

/// A role with its permission keys.
struct RoleWithPermissions {
    id: Uuid,
    name: String,
    description: Option<String>,
    permissions: Vec<String>,
}

fn role_has_admin_perms(role: &RoleWithPermissions) -> bool {
    role.permissions.iter().any(|p| p.starts_with("admin."))
}

async fn list_roles(
    State(state): State<AppState>,
    user: UserContext,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.roles.read")?;
    let roles = load_roles(&state.db).await.map_err(ApiError::from)?;
    let roles: Vec<Value> = roles
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.name,
                "description": r.description,
                "permissions": r.permissions,
            })
        })
        .collect();
    Ok(Json(json!({ "roles": roles })))
}

async fn load_roles(pool: &sqlx::PgPool) -> Result<Vec<RoleWithPermissions>, sqlx::Error> {
    let role_rows = sqlx::query("SELECT id, name, description FROM roles ORDER BY name")
        .fetch_all(pool)
        .await?;
    let perm_rows = sqlx::query(
        r#"
        SELECT rp.role_id, p.key FROM role_permissions rp
        JOIN permissions p ON p.id = rp.permission_id
        ORDER BY p.key
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut roles: Vec<RoleWithPermissions> = role_rows
        .into_iter()
        .map(|row| RoleWithPermissions {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            permissions: Vec::new(),
        })
        .collect();

    for row in perm_rows {
        let role_id: Uuid = row.get("role_id");
        let key: String = row.get("key");
        if let Some(role) = roles.iter_mut().find(|r| r.id == role_id) {
            role.permissions.push(key);
        }
    }
    Ok(roles)
}

#[derive(Debug, Deserialize)]
pub struct RoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission_keys: Option<Vec<String>>,
}

async fn create_role(
    State(state): State<AppState>,
    actor: UserContext,
    Json(req): Json<RoleRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.roles.update")?;
    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::new(ApiErrorKind::InvalidRequest("name is required".into())))?;
    let keys = req.permission_keys.unwrap_or_default();

    let mut tx = state.db.begin().await.map_err(ApiError::from)?;

    let (id,): (Uuid,) =
        sqlx::query_as("INSERT INTO roles (name, description) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(req.description.as_deref())
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db) if db.is_unique_violation() => {
                    ApiError::new(ApiErrorKind::Conflict("role name already exists".into()))
                }
                other => ApiError::from(other),
            })?;

    replace_role_permissions(&mut tx, id, &keys).await?;
    audit::write(
        &mut *tx,
        audit::AuditEntry {
            actor_user_id: actor.user_id,
            action: audit::action::ROLE_CREATED,
            target_type: Some("role"),
            target_id: Some(id),
            old_value: None,
            new_value: Some(json!({ "name": name, "permissions": keys })),
            ip_hash: Some(&actor.ip_hash),
        },
    )
    .await
    .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;
    Ok(Json(json!({ "id": id })))
}

async fn update_role(
    State(state): State<AppState>,
    actor: UserContext,
    Path(id): Path<Uuid>,
    Json(req): Json<RoleRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.roles.update")?;

    let role = load_roles(&state.db)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::new(ApiErrorKind::NotFound))?;

    // Final-admin protection: removing the last admin-capable role's admin
    // permissions is forbidden.
    if let Some(keys) = &req.permission_keys {
        let would_keep_admin = keys.iter().any(|k| k.starts_with("admin."));
        if role_has_admin_perms(&role) && !would_keep_admin {
            let admin_count = count_admin_capable_roles(&state.db)
                .await
                .map_err(ApiError::from)?;
            if admin_count <= 1 {
                return Err(ApiError::new(ApiErrorKind::Conflict(
                    "cannot remove admin permissions from the last admin role".into(),
                )));
            }
        }
    }

    let mut tx = state.db.begin().await.map_err(ApiError::from)?;

    let (name, description) = (
        req.name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| role.name.clone()),
        req.description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| role.description.clone().unwrap_or_default()),
    );
    sqlx::query("UPDATE roles SET name = $2, description = $3, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&name)
        .bind(&description)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;

    let mut events = Vec::new();
    if let Some(keys) = &req.permission_keys {
        replace_role_permissions(&mut tx, id, keys).await?;
        audit::write(
            &mut *tx,
            audit::AuditEntry {
                actor_user_id: actor.user_id,
                action: audit::action::ROLE_PERMISSIONS_CHANGED,
                target_type: Some("role"),
                target_id: Some(id),
                old_value: Some(json!({ "permissions": role.permissions })),
                new_value: Some(json!({ "permissions": keys })),
                ip_hash: Some(&actor.ip_hash),
            },
        )
        .await
        .map_err(ApiError::from)?;
        events.push(json!({ "permissions": keys }));
    }
    events.push(json!({ "name": name, "description": description }));

    audit::write(
        &mut *tx,
        audit::AuditEntry {
            actor_user_id: actor.user_id,
            action: audit::action::ROLE_UPDATED,
            target_type: Some("role"),
            target_id: Some(id),
            old_value: Some(json!({ "name": role.name, "permissions": role.permissions })),
            new_value: Some(json!({ "changed": events })),
            ip_hash: Some(&actor.ip_hash),
        },
    )
    .await
    .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;
    invalidate_role_policy(&state, id).await;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_role(
    State(state): State<AppState>,
    actor: UserContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.roles.update")?;

    let role = load_roles(&state.db)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::new(ApiErrorKind::NotFound))?;

    if role_has_admin_perms(&role) {
        let admin_count = count_admin_capable_roles(&state.db)
            .await
            .map_err(ApiError::from)?;
        if admin_count <= 1 {
            return Err(ApiError::new(ApiErrorKind::Conflict(
                "cannot delete the last admin role".into(),
            )));
        }
    }

    let (users_with_role,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM users WHERE role_id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .map_err(ApiError::from)?;
    if users_with_role > 0 {
        return Err(ApiError::new(ApiErrorKind::Conflict(format!(
            "role is assigned to {users_with_role} user(s)"
        ))));
    }

    let mut tx = state.db.begin().await.map_err(ApiError::from)?;
    sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    audit::write(
        &mut *tx,
        audit::AuditEntry {
            actor_user_id: actor.user_id,
            action: audit::action::ROLE_DELETED,
            target_type: Some("role"),
            target_id: Some(id),
            old_value: Some(json!({ "name": role.name })),
            new_value: None,
            ip_hash: Some(&actor.ip_hash),
        },
    )
    .await
    .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;

    invalidate_role_policy(&state, id).await;
    Ok(Json(json!({ "ok": true })))
}

async fn replace_role_permissions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    role_id: Uuid,
    keys: &[String],
) -> Result<(), ApiError> {
    // Validate every key exists.
    let (found,): (i64,) = sqlx::query_as("SELECT count(*) FROM permissions WHERE key = ANY($1)")
        .bind(keys)
        .fetch_one(&mut **tx)
        .await
        .map_err(ApiError::from)?;
    if found != keys.len() as i64 {
        return Err(ApiError::new(ApiErrorKind::InvalidRequest(
            "one or more permission keys do not exist".into(),
        )));
    }

    sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
        .bind(role_id)
        .execute(&mut **tx)
        .await
        .map_err(ApiError::from)?;
    if !keys.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO role_permissions (role_id, permission_id)
            SELECT $1, id FROM permissions WHERE key = ANY($2)
            "#,
        )
        .bind(role_id)
        .bind(keys)
        .execute(&mut **tx)
        .await
        .map_err(ApiError::from)?;
    }
    Ok(())
}

async fn count_admin_capable_roles(pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT count(DISTINCT rp.role_id)
        FROM role_permissions rp
        JOIN permissions p ON p.id = rp.permission_id
        WHERE p.key LIKE 'admin.%'
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Delete the role-level policy cache keys after a policy-affecting change.
pub async fn invalidate_role_policy(state: &AppState, role_id: Uuid) {
    let key = format!("weave:policy:role:{role_id}");
    let mut conn = state.redis.connection();
    let _ = redis::cmd("DEL")
        .arg(key)
        .query_async::<()>(&mut conn)
        .await;
}
