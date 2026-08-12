//! PostgreSQL connection pool and migration runner.
//!
//! Migrations live in `backend/migrations/` and are applied at startup via
//! `sqlx::migrate!`. The application never silently creates tables; schema
//! changes go through explicit migration files.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Open a connection pool and apply pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("database connected and migrations applied");
    Ok(pool)
}

/// Cheap liveness probe used by `/health/ready`.
pub async fn ping(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}
