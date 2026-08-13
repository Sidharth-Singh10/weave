//! Admin rate-limit / quota policy routes.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::audit;
use crate::auth::middleware::{UserContext, require_permission};
use crate::error::{ApiError, ApiErrorKind};
use crate::policy::{Limits, RawRule};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/policies", get(get_policies))
        .route(
            "/api/admin/policies/roles/{role_id}",
            get(get_policies).post(patch_role_policy),
        )
        .route(
            "/api/admin/policies/users/{user_id}",
            get(get_policies).post(patch_user_policy),
        )
}

#[derive(Debug, Deserialize)]
pub struct PolicyUpdateRequest {
    #[serde(default)]
    pub limits: Limits,
}

/// Full policy view in the admin-API shape (inheritance-friendly).
async fn get_policies(
    State(state): State<AppState>,
    user: UserContext,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.policies.read")?;
    let policies = load_all_policies(&state.db).await.map_err(ApiError::from)?;

    let mut global = Limits::default();
    let mut roles: Vec<Value> = Vec::new();
    let mut users: Vec<Value> = Vec::new();

    for p in policies {
        let limits_json = p.limits.to_owned();
        match p.scope_type.as_str() {
            "global" if p.endpoint.is_none() => global = limits_json,
            "role" => {
                let role_id = p.role_id.unwrap();
                let role = p.role_name.unwrap_or_default();
                if p.endpoint.is_none() {
                    roles.push(json!({
                        "role_id": role_id,
                        "role": role,
                        "limits": limits_json,
                        "endpoints": {},
                    }));
                } else if let Some(last) = roles.last_mut() {
                    let endpoint = p.endpoint.unwrap_or_default();
                    last["endpoints"][endpoint] = serde_json::to_value(limits_json).unwrap();
                }
            }
            "user" => users.push(json!({
                "user_id": p.user_id.unwrap(),
                "email": p.user_email.unwrap_or_default(),
                "role": p.user_role_name.unwrap_or_default(),
                "overrides": limits_json,
            })),
            _ => {}
        }
    }

    Ok(Json(json!({
        "global": global,
        "roles": roles,
        "users": users,
    })))
}

async fn patch_role_policy(
    State(state): State<AppState>,
    actor: UserContext,
    Path(role_id): Path<Uuid>,
    Json(req): Json<PolicyUpdateRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.policies.update")?;
    ensure_role_exists(&state.db, role_id).await?;

    let mut tx = state.db.begin().await.map_err(ApiError::from)?;
    let policy_id = upsert_policy(&mut tx, "role", Some(role_id), None).await?;
    replace_rules(&mut tx, policy_id, &req.limits).await?;
    audit::write(
        &mut *tx,
        audit::AuditEntry {
            actor_user_id: actor.user_id,
            action: audit::action::POLICY_ROLE_UPDATED,
            target_type: Some("role"),
            target_id: Some(role_id),
            old_value: None,
            new_value: Some(json!({ "limits": req.limits })),
            ip_hash: Some(&actor.ip_hash),
        },
    )
    .await
    .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;

    invalidate_policy(&state, "role", role_id).await;
    Ok(Json(json!({ "ok": true })))
}

async fn patch_user_policy(
    State(state): State<AppState>,
    actor: UserContext,
    Path(user_id): Path<Uuid>,
    Json(req): Json<PolicyUpdateRequest>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&actor, "admin.policies.update")?;
    ensure_user_exists(&state.db, user_id).await?;

    let mut tx = state.db.begin().await.map_err(ApiError::from)?;

    if req.limits.is_empty() {
        // Removing the override deletes the user's generic policy row.
        let res = sqlx::query(
            "DELETE FROM rate_limit_policies WHERE scope_type = 'user' AND user_id = $1 AND endpoint IS NULL",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
        audit::write(
            &mut *tx,
            audit::AuditEntry {
                actor_user_id: actor.user_id,
                action: audit::action::POLICY_USER_OVERRIDE_REMOVED,
                target_type: Some("user"),
                target_id: Some(user_id),
                old_value: None,
                new_value: None,
                ip_hash: Some(&actor.ip_hash),
            },
        )
        .await
        .map_err(ApiError::from)?;
        let removed = res.rows_affected() > 0;
        tx.commit().await.map_err(ApiError::from)?;
        invalidate_policy(&state, "user", user_id).await;
        return Ok(Json(json!({ "ok": true, "removed": removed })));
    }

    let policy_id = upsert_policy(&mut tx, "user", None, Some(user_id)).await?;
    replace_rules(&mut tx, policy_id, &req.limits).await?;
    audit::write(
        &mut *tx,
        audit::AuditEntry {
            actor_user_id: actor.user_id,
            action: audit::action::POLICY_USER_OVERRIDE_UPDATED,
            target_type: Some("user"),
            target_id: Some(user_id),
            old_value: None,
            new_value: Some(json!({ "limits": req.limits })),
            ip_hash: Some(&actor.ip_hash),
        },
    )
    .await
    .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;

    invalidate_policy(&state, "user", user_id).await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct Policy {
    scope_type: String,
    role_id: Option<Uuid>,
    user_id: Option<Uuid>,
    endpoint: Option<String>,
    role_name: Option<String>,
    user_email: Option<String>,
    user_role_name: Option<String>,
    limits: Limits,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct PolicyKey {
    scope_type: String,
    role_id: Option<Uuid>,
    user_id: Option<Uuid>,
    endpoint: Option<String>,
}

async fn load_all_policies(pool: &sqlx::PgPool) -> Result<Vec<Policy>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT p.scope_type, p.role_id, p.user_id, p.endpoint,
               r.name AS role_name, u.email AS user_email,
               u2.name AS user_role_name,
               rr.metric, rr.time_window, rr.limit_value
        FROM rate_limit_policies p
        LEFT JOIN roles r ON r.id = p.role_id
        LEFT JOIN users u ON u.id = p.user_id
        LEFT JOIN roles u2 ON u2.id = u.role_id
        LEFT JOIN rate_limit_rules rr ON rr.policy_id = p.id
        ORDER BY p.scope_type, p.endpoint NULLS FIRST, p.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Accumulate every rule per policy, then convert to Limits once.
    struct Accum {
        policy: Policy,
        rules: Vec<RawRule>,
    }

    let mut acc: Vec<Accum> = Vec::new();
    for row in rows {
        let key = PolicyKey {
            scope_type: row.get("scope_type"),
            role_id: row.get("role_id"),
            user_id: row.get("user_id"),
            endpoint: row.get("endpoint"),
        };

        let metric: Option<String> = row.get("metric");
        if let Some(pos) = acc.iter().position(|a| {
            a.policy.scope_type == key.scope_type
                && a.policy.role_id == key.role_id
                && a.policy.user_id == key.user_id
                && a.policy.endpoint == key.endpoint
        }) {
            if let Some(metric) = metric {
                acc[pos].rules.push(RawRule {
                    metric,
                    time_window: row.get("time_window"),
                    limit: row.get("limit_value"),
                });
            }
            continue;
        }

        let mut rules = Vec::new();
        if let Some(metric) = metric {
            rules.push(RawRule {
                metric,
                time_window: row.get("time_window"),
                limit: row.get("limit_value"),
            });
        }
        acc.push(Accum {
            policy: Policy {
                scope_type: key.scope_type,
                role_id: key.role_id,
                user_id: key.user_id,
                endpoint: key.endpoint,
                role_name: row.get("role_name"),
                user_email: row.get("user_email"),
                user_role_name: row.get("user_role_name"),
                limits: Limits::default(),
            },
            rules,
        });
    }

    Ok(acc
        .into_iter()
        .map(|mut a| {
            a.policy.limits = Limits::from_rules(&a.rules);
            a.policy
        })
        .collect())
}

async fn ensure_role_exists(pool: &sqlx::PgPool, role_id: Uuid) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM roles WHERE id = $1)")
        .bind(role_id)
        .fetch_one(pool)
        .await
        .map_err(ApiError::from)?;
    if exists {
        Ok(())
    } else {
        Err(ApiError::new(ApiErrorKind::NotFound))
    }
}

async fn ensure_user_exists(pool: &sqlx::PgPool, user_id: Uuid) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(ApiError::from)?;
    if exists {
        Ok(())
    } else {
        Err(ApiError::new(ApiErrorKind::NotFound))
    }
}

/// Find or create the generic (endpoint NULL) policy row for a scope target.
async fn upsert_policy(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope_type: &str,
    role_id: Option<Uuid>,
    user_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    let (id,): (Uuid,) = match scope_type {
        "role" => {
            sqlx::query_as(
                r#"
            INSERT INTO rate_limit_policies (scope_type, role_id, endpoint)
            VALUES ('role', $1, NULL)
            ON CONFLICT (scope_type, role_id, (COALESCE(endpoint, ''))) WHERE scope_type = 'role'
            DO UPDATE SET updated_at = now()
            RETURNING id
            "#,
            )
            .bind(role_id)
            .fetch_one(&mut **tx)
            .await
        }
        "user" => {
            sqlx::query_as(
                r#"
            INSERT INTO rate_limit_policies (scope_type, user_id, endpoint)
            VALUES ('user', $1, NULL)
            ON CONFLICT (scope_type, user_id, (COALESCE(endpoint, ''))) WHERE scope_type = 'user'
            DO UPDATE SET updated_at = now()
            RETURNING id
            "#,
            )
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await
        }
        _ => unreachable!(),
    }
    .map_err(ApiError::from)?;
    Ok(id)
}

/// Replace a policy's rules with those derived from `limits`.
async fn replace_rules(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy_id: Uuid,
    limits: &Limits,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM rate_limit_rules WHERE policy_id = $1")
        .bind(policy_id)
        .execute(&mut **tx)
        .await
        .map_err(ApiError::from)?;

    for rule in limits.rules() {
        sqlx::query(
            "INSERT INTO rate_limit_rules (policy_id, metric, time_window, limit_value)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(policy_id)
        .bind(rule.metric)
        .bind(rule.time_window)
        .bind(rule.limit)
        .execute(&mut **tx)
        .await
        .map_err(ApiError::from)?;
    }
    Ok(())
}

async fn invalidate_policy(state: &AppState, scope: &str, id: Uuid) {
    let key = format!("weave:policy:{scope}:{id}");
    let mut conn = state.redis.connection();
    let _ = redis::cmd("DEL")
        .arg(key)
        .query_async::<()>(&mut conn)
        .await;
}
