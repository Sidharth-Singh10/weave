//! Admin API: users, roles, policies, and audit log.
//!
//! Every route requires an explicit admin permission (server-side enforced);
//! never rely on the frontend hiding buttons.

pub mod policies;
pub mod roles;
pub mod users;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::audit as audit_log;
use crate::auth::middleware::{UserContext, require_permission};
use crate::error::ApiError;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(users::routes())
        .merge(roles::routes())
        .merge(policies::routes())
        .route("/api/admin/audit", get(list_audit))
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
}

async fn list_audit(
    State(state): State<AppState>,
    user: UserContext,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Value>, ApiError> {
    require_permission(&user, "admin.audit.read")?;

    let since = q
        .since
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&chrono::Utc));
    let until = q
        .until
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&chrono::Utc));

    let page = audit_log::list(
        &state.db,
        q.limit.unwrap_or(50),
        q.cursor.as_deref(),
        q.actor.as_deref(),
        q.action.as_deref(),
        q.target_type.as_deref(),
        since,
        until,
    )
    .await
    .map_err(ApiError::from)?;

    Ok(Json(json!({
        "items": page.entries,
        "next_cursor": page.next_cursor,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::config::Config;
    use crate::db;
    use crate::llm::OpenCodeClient;
    use crate::redis_store::Redis;

    struct TestCtx {
        app: Router,
        db: sqlx::PgPool,
    }

    /// Acquires the shared DB lock so parallel tests cannot corrupt the shared
    /// dev database's fixtures.
    async fn setup() -> Option<(TestCtx, std::sync::MutexGuard<'static, ()>)> {
        let guard = crate::testutil::db_lock::LOCK.lock().unwrap();
        let db_url = std::env::var("DATABASE_URL").ok()?;
        let redis_url = std::env::var("REDIS_URL").ok()?;
        let pool = db::connect(&db_url).await.ok()?;
        let redis = Redis::connect(&redis_url).await.ok()?;
        let mut config = Config::from_env();
        config.auth_stub = true;
        config.session_cookie_name = "weave_session".to_string();
        let state = AppState {
            config: std::sync::Arc::new(config),
            llm: std::sync::Arc::new(OpenCodeClient::from_env()),
            db: pool.clone(),
            redis,
            oidc: std::sync::Arc::new(crate::auth::oauth::MockOidc::new(
                "https://idp.example/a",
                crate::auth::oauth::OidcIdentity {
                    subject: "sub-admin-test".to_string(),
                    email: "admin@test.com".to_string(),
                    name: None,
                    picture: None,
                },
            )),
        };
        Some((
            TestCtx {
                app: crate::auth::routes().merge(routes()).with_state(state),
                db: pool,
            },
            guard,
        ))
    }

    async fn login(app: &Router, email: &str) -> String {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/test/login")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"email":"{email}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        cookie.split(';').next().unwrap().to_string()
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        cookie: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(c) = cookie {
            builder = builder.header("cookie", c);
        }
        let res = app
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, value)
    }

    async fn body_json(res: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    #[tokio::test]
    async fn admin_permissions_and_audit() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let admin_cookie = login(&ctx.app, "admin@test.com").await;
        let member_cookie = login(&ctx.app, "member@test.com").await;

        // Promote the admin user deterministically (bootstrap may already be
        // consumed by the shared dev database).
        sqlx::query(
            "UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') WHERE email = $1",
        )
        .bind("admin@test.com")
        .execute(&ctx.db)
        .await
        .unwrap();

        // Admin can list users.
        let (status, body) = call(&ctx.app, "GET", "/api/admin/users", Some(&admin_cookie)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["items"].is_array());

        // Member is forbidden from admin routes.
        let (status, _) = call(&ctx.app, "GET", "/api/admin/users", Some(&member_cookie)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = call(&ctx.app, "GET", "/api/admin/audit", Some(&member_cookie)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = call(&ctx.app, "GET", "/api/admin/policies", Some(&member_cookie)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Unauthenticated is rejected.
        let (status, _) = call(&ctx.app, "GET", "/api/admin/users", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Admin reads the policies view.
        let (status, body) =
            call(&ctx.app, "GET", "/api/admin/policies", Some(&admin_cookie)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["global"]["requests_per_minute"], 30);

        // Admin demotes the member to guest -> audit entry written.
        let (member_id, guest_role_id): (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
            "SELECT u.id, (SELECT id FROM roles WHERE name = 'guest') FROM users u WHERE u.email = $1",
        )
        .bind("member@test.com")
        .fetch_one(&ctx.db)
        .await
        .unwrap();

        let patch = serde_json::json!({ "role_id": guest_role_id }).to_string();
        let res = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/admin/users/{member_id}"))
                    .header("cookie", &admin_cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(patch))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_json(res).await["ok"], true);

        let (status, body) = call(&ctx.app, "GET", "/api/admin/audit", Some(&admin_cookie)).await;
        assert_eq!(status, StatusCode::OK);
        let actions: Vec<String> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["action"].as_str().map(str::to_string))
            .collect();
        assert!(
            actions.contains(&"user.role_changed".to_string()),
            "actions: {actions:?}"
        );
    }

    #[tokio::test]
    async fn last_admin_is_protected() {
        let Some((ctx, _guard)) = setup().await else {
            eprintln!("skipping: DATABASE_URL/REDIS_URL not set or unavailable");
            return;
        };
        let _admin_cookie = login(&ctx.app, "admin@test.com").await;
        let _member_cookie = login(&ctx.app, "member@test.com").await;
        let super_cookie = login(&ctx.app, "super@test.com").await;

        // Make the shared dev DB deterministic: promote admin@test.com, demote
        // any other active admins so admin@test.com is the sole user with the
        // `admin` role.
        sqlx::query(
            "UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') WHERE email = $1",
        )
        .bind("admin@test.com")
        .execute(&ctx.db)
        .await
        .unwrap();
        sqlx::query(
            r#"UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'member')
               WHERE status = 'active' AND role_id = (SELECT id FROM roles WHERE name = 'admin')
                 AND email <> 'admin@test.com'"#,
        )
        .execute(&ctx.db)
        .await
        .unwrap();

        // The actor holds admin.users.update through a separate `superadmin`
        // role, so they are not the admin they are trying to demote.
        sqlx::query("INSERT INTO roles (name, description) VALUES ('superadmin', 'test') ON CONFLICT (name) DO NOTHING")
            .execute(&ctx.db)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO role_permissions (role_id, permission_id)
               SELECT (SELECT id FROM roles WHERE name='superadmin'),
                      (SELECT id FROM permissions WHERE key='admin.users.update')
               ON CONFLICT DO NOTHING"#,
        )
        .execute(&ctx.db)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE users SET role_id = (SELECT id FROM roles WHERE name='superadmin') WHERE email = 'super@test.com'",
        )
        .execute(&ctx.db)
        .await
        .unwrap();

        let (admin_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind("admin@test.com")
            .fetch_one(&ctx.db)
            .await
            .unwrap();
        let (member_role_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM roles WHERE name = 'member'")
                .fetch_one(&ctx.db)
                .await
                .unwrap();

        // The superadmin actor cannot demote the last admin-role user.
        let patch = serde_json::json!({ "role_id": member_role_id }).to_string();
        let res = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/admin/users/{admin_id}"))
                    .header("cookie", &super_cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(patch))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(body_json(res).await["error"]["code"], "conflict");

        // Nor can they disable the last active admin.
        let patch = serde_json::json!({ "status": "disabled" }).to_string();
        let res = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/admin/users/{admin_id}"))
                    .header("cookie", &super_cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(patch))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }
}
