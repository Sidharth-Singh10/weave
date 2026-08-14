//! Database bootstrap + pool.
//!
//! Ensures the `weave_mcp` database exists (connecting to the maintenance
//! `postgres` database first), then connects and applies migrations.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Connect to the maintenance DB and create the target database if missing,
/// then connect to it and apply pending migrations.
pub async fn ensure_and_migrate(db_url: &str) -> anyhow::Result<PgPool> {
    let target_db = url::Url::parse(db_url)?
        .path_segments()
        .and_then(|segments| segments.last())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "weave_mcp".to_string());

    // Maintenance connection to the always-present `postgres` database.
    let mut admin_url = url::Url::parse(db_url)?;
    admin_url.set_path("/postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(admin_url.as_str())
        .await?;

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&target_db)
            .fetch_one(&admin_pool)
            .await?;
    if !exists {
        let ddl = format!("CREATE DATABASE {}", quote_ident(&target_db));
        sqlx::query(&ddl).execute(&admin_pool).await?;
        tracing::info!(database = %target_db, "created weave_mcp database");
    }
    admin_pool.close().await;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(db_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("weave_mcp database ready");
    Ok(pool)
}
