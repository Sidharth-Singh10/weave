//! weave-mcp: a personal knowledge/memory MCP server.
//!
//! Agents (Hermes, Claude Desktop, Codex, …) connect over stdio or Streamable
//! HTTP and use the tools to store notes/files and retrieve knowledge from the
//! graph. The transport is selected with `WEAVE_MCP_TRANSPORT`:
//!
//! - `stdio` (default): launched by the MCP client as a subprocess.
//! - `http`: serves the Streamable HTTP endpoint at `/mcp` (built with the
//!   `http` feature); `WEAVE_MCP_HTTP_ADDR` controls the bind address.

use std::sync::Arc;

use rmcp::ServiceExt;
use weave_mcp::config::Config;
use weave_mcp::server::MemoryServer;
use weave_mcp::{db, embed};

#[cfg(feature = "http")]
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

fn transport() -> String {
    std::env::var("WEAVE_MCP_TRANSPORT")
        .unwrap_or_else(|_| "stdio".to_string())
        .to_lowercase()
}

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
    let embedder = embed::build_embedder();

    tracing::info!(
        llm_mode = if llm.available() { "opencode" } else { "mock" },
        "weave-mcp starting"
    );

    let service = MemoryServer::new(pool, Arc::new(config), llm, embedder);

    if transport() == "http" {
        serve_http(service).await
    } else {
        serve_stdio(service).await
    }
}

/// stdio transport — the MCP client spawns this binary and talks over
/// stdin/stdout. This is the historical default and remains supported.
async fn serve_stdio(service: MemoryServer) -> anyhow::Result<()> {
    use rmcp::transport::stdio;

    let server = service.serve(stdio()).await?;
    tracing::info!("weave-mcp ready on stdio");
    server.waiting().await?;
    Ok(())
}

#[cfg(not(feature = "http"))]
async fn serve_http(_service: MemoryServer) -> anyhow::Result<()> {
    anyhow::bail!(
        "weave-mcp was built without the `http` feature; rebuild with `--features http` to serve Streamable HTTP"
    )
}

/// Streamable HTTP transport — serves `/mcp` so remote MCP clients (Hermes
/// gateways, Claude Desktop, ...) can connect over HTTP. Only compiled when the
/// `http` feature is enabled.
#[cfg(feature = "http")]
async fn serve_http(service: MemoryServer) -> anyhow::Result<()> {
    let addr = std::env::var("WEAVE_MCP_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8010".to_string());

    let streamable = StreamableHttpService::new(
        move || Ok(service.clone()),
        LocalSessionManager::default().into(),
        http_server_config(),
    );

    let router = axum::Router::new().nest_service("/mcp", streamable);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "weave-mcp ready on streamable HTTP at /mcp");
    axum::serve(listener, router).await?;
    Ok(())
}

/// Build the streamable HTTP server config.
///
/// `WEAVE_MCP_ALLOWED_HOSTS` (comma-separated) optionally restricts which `Host`
/// headers are accepted (e.g. `weave-mcp,127.0.0.1`). When unset the check is
/// disabled so the server answers regardless of how clients address it: agents
/// on `weave_net` use `Host: weave-mcp`, the host-native agent uses
/// `127.0.0.1`, and containers on other networks use the host gateway IP.
#[cfg(feature = "http")]
fn http_server_config() -> StreamableHttpServerConfig {
    match std::env::var("WEAVE_MCP_ALLOWED_HOSTS") {
        Ok(v) if !v.trim().is_empty() => StreamableHttpServerConfig::default()
            .with_allowed_hosts(v.split(',').map(|h| h.trim().to_string())),
        _ => StreamableHttpServerConfig::default().disable_allowed_hosts(),
    }
}
