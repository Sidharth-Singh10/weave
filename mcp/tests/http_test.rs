//! Streamable HTTP transport end-to-end test (gated on the `http` feature).
//!
//! Spawns the `weave-mcp` binary in HTTP mode on an ephemeral port, connects
//! with an rmcp HTTP client, and drives a `remember` / `get_note` round-trip.
//! DB-backed like `memory_test.rs`: skipped when no Postgres is reachable.

#![cfg(feature = "http")]

use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, ContentBlock, Implementation};
use rmcp::transport::StreamableHttpClientTransport;
use tokio::process::Command;

fn mcp_db_url() -> Option<String> {
    if let Ok(url) = std::env::var("WEAVE_MCP_DATABASE_URL") {
        return Some(url);
    }
    let base = std::env::var("DATABASE_URL").ok()?;
    let mut parsed = url::Url::parse(&base).ok()?;
    parsed.set_path("/weave_mcp");
    Some(parsed.to_string())
}

/// Grab an ephemeral free port on loopback.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("bind ephemeral port")
}

#[tokio::test]
async fn streamable_http_roundtrip_via_mcp_protocol() {
    let Some(db_url) = mcp_db_url() else {
        eprintln!("skipping: no reachable database");
        return;
    };

    let port = free_port();
    let addr = format!("127.0.0.1:{port}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_weave-mcp"))
        .env("WEAVE_MCP_TRANSPORT", "http")
        .env("WEAVE_MCP_HTTP_ADDR", &addr)
        .env("WEAVE_MCP_DATABASE_URL", &db_url)
        .env_remove("OPENCODE_API_KEY")
        .kill_on_drop(true)
        .spawn()
        .expect("spawn http mcp server");

    // Wait for the server to accept an initialize handshake.
    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("weave-mcp-http-test", "0.0.1"),
    );
    let uri = format!("http://{addr}/mcp");
    let client = retry(|| async {
        let transport = StreamableHttpClientTransport::from_uri(uri.as_str());
        client_info.clone().serve(transport).await
    })
    .await
    .expect("connect to streamable http mcp server");

    // The tools are advertised over HTTP too.
    let tools = client.list_all_tools().await.expect("list tools");
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    for expected in ["remember", "get_note", "search", "recall_memory"] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing {expected}: {names:?}"
        );
    }

    // remember a note -> get_note round-trips the content.
    let result = client
        .call_tool(
            CallToolRequestParams::new("remember").with_arguments(rmcp::object!({
                "text": "Hermione studies at Hogwarts and is afraid of spiders.",
            })),
        )
        .await
        .expect("call remember");

    let text: String = result
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let parsed: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({"raw": text}));
    let note_id = parsed["note_id"]
        .as_str()
        .expect("remember result contains note_id");

    let note = client
        .call_tool(
            CallToolRequestParams::new("get_note").with_arguments(rmcp::object!({
                "id": note_id,
            })),
        )
        .await
        .expect("call get_note");
    let note_text: String = note
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(note_text.contains("Hermione"));

    // Cleanup and shut the server down.
    client
        .call_tool(
            CallToolRequestParams::new("delete_note").with_arguments(rmcp::object!({
                "id": note_id,
            })),
        )
        .await
        .expect("call delete_note");

    client.cancel().await.ok();
    child.kill().await.ok();
}

/// Retry a fallible future until it succeeds or the timeout elapses.
async fn retry<T, E: std::fmt::Debug, F, Fut>(mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let _ = err;
            }
            Err(err) => return Err(err),
        }
    }
}
