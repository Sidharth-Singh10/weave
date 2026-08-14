//! weave-mcp: a personal knowledge/memory MCP server.
//!
//! Agents (Hermes, Claude Desktop, Codex, …) connect over stdio and use the
//! tools to store notes/files and retrieve knowledge from the graph.

use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::transport::stdio;
use weave_mcp::config::Config;
use weave_mcp::db;
use weave_mcp::server::MemoryServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weave_mcp=info,weave_core=warn,info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let pool = db::ensure_and_migrate(&config.database_url).await?;
    let llm = Arc::new(weave_core::llm::OpenCodeClient::from_env());

    tracing::info!(
        llm_mode = if llm.available() { "opencode" } else { "mock" },
        "weave-mcp starting"
    );

    let service = MemoryServer::new(pool, Arc::new(config), llm);
    let server = service.serve(stdio()).await?;
    tracing::info!("weave-mcp ready on stdio");
    server.waiting().await?;
    Ok(())
}
