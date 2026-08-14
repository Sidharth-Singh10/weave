//! Integration tests for the weave-mcp storage pipeline and MCP protocol.
//!
//! DB-backed tests are gated on a reachable Postgres (`DATABASE_URL` or
//! `WEAVE_MCP_DATABASE_URL`); the target database is `weave_mcp` (created by
//! `db::ensure_and_migrate`). The LLM is forced into deterministic mock mode
//! so tests never hit the network.

use std::sync::Arc;

use weave_core::llm::OpenCodeClient;
use weave_mcp::{db, embed, ingest, store};

/// Serializes DB-backed integration tests so they cannot corrupt each other's
/// fixtures (they share the `weave_mcp` database).
static DB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn stub_embedder() -> Arc<dyn embed::Embedder> {
    Arc::new(embed::StubEmbedder)
}

/// Resolve the mcp database URL: `WEAVE_MCP_DATABASE_URL` wins, else
/// `DATABASE_URL` with the database swapped to `weave_mcp`.
fn mcp_db_url() -> Option<String> {
    if let Ok(url) = std::env::var("WEAVE_MCP_DATABASE_URL") {
        return Some(url);
    }
    let base = std::env::var("DATABASE_URL").ok()?;
    let mut parsed = url::Url::parse(&base).ok()?;
    parsed.set_path("/weave_mcp");
    Some(parsed.to_string())
}

async fn pool() -> Option<sqlx::PgPool> {
    let url = mcp_db_url()?;
    db::ensure_and_migrate(&url).await.ok()
}

#[tokio::test]
async fn ingest_pipeline_persists_graph_with_provenance() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let llm = Arc::new(OpenCodeClient::mock());
    let embedder = stub_embedder();

    let result = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "PerOXO uses Rust and ScyllaDB.",
        "note",
        &["work".to_string()],
        "user",
        None,
    )
    .await
    .expect("ingest");

    assert!(
        result.total_entities >= 3,
        "entities: {}",
        result.total_entities
    );
    assert!(
        result.total_relations >= 2,
        "relations: {}",
        result.total_relations
    );

    // The note is stored with its tags.
    let note = store::get_note(&pool, result.note_id)
        .await
        .unwrap()
        .expect("note");
    assert_eq!(note.content, "PerOXO uses Rust and ScyllaDB.");
    assert_eq!(note.tags, vec!["work"]);

    // The note was embedded (pgvector column written).
    let embedded: bool =
        sqlx::query_scalar("SELECT embedding IS NOT NULL FROM notes WHERE id = $1")
            .bind(result.note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(embedded, "note embedding must be persisted");

    // Provenance: the note references the extracted entities.
    let entities = store::entities_for_note(&pool, result.note_id)
        .await
        .unwrap();
    let labels: Vec<String> = entities.iter().map(|e| e.label.to_lowercase()).collect();
    for expected in ["peroxo", "rust", "scylladb"] {
        assert!(
            labels.iter().any(|l| l == expected),
            "missing {expected} in {labels:?}"
        );
    }

    // Provenance: relations created by this note are linked.
    let relations = store::relations_for_note(&pool, result.note_id)
        .await
        .unwrap();
    assert!(!relations.is_empty());

    // Re-ingesting the same text must reuse existing entities (no dupes).
    let again = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "PerOXO uses Rust and ScyllaDB.",
        "note",
        &[],
        "user",
        None,
    )
    .await
    .expect("re-ingest");
    assert_eq!(again.total_entities, result.total_entities);

    store::delete_note(&pool, result.note_id).await.unwrap();
    store::delete_note(&pool, again.note_id).await.unwrap();
}

#[tokio::test]
async fn recall_roundtrip_via_mcp_protocol() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let _ = pool;

    use rmcp::ServiceExt;
    use rmcp::model::{CallToolRequestParams, ContentBlock};
    use rmcp::transport::TokioChildProcess;
    use tokio::process::Command;

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_weave-mcp"));
    cmd.env("WEAVE_MCP_DATABASE_URL", mcp_db_url().unwrap())
        .env_remove("OPENCODE_API_KEY");

    let child = TokioChildProcess::new(cmd).expect("build child transport");
    let client = ().serve(child).await.expect("spawn mcp server");

    // The tools are advertised.
    let tools = client.list_all_tools().await.expect("list tools");
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    for expected in ["remember", "list_notes", "get_note", "delete_note"] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing {expected}: {names:?}"
        );
    }

    // remember a note -> note_id in the result.
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

    // get_note round-trips the content.
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

    // --- Phase 2: retrieval tools ---------------------------------------
    // search finds the note by full-text match.
    let search = client
        .call_tool(
            CallToolRequestParams::new("search").with_arguments(rmcp::object!({
                "query": "Hogwarts",
            })),
        )
        .await
        .expect("call search");
    let search_text: String = search
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        search_text.contains("Hermione"),
        "search should find the note: {search_text}"
    );

    // get_node resolves the entity and its relations.
    let node = client
        .call_tool(
            CallToolRequestParams::new("get_node").with_arguments(rmcp::object!({
                "label": "Hermione",
            })),
        )
        .await
        .expect("call get_node");
    let node_text: String = node
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        node_text.contains("Hogwarts"),
        "get_node should show relations: {node_text}"
    );

    // get_related expands the neighborhood.
    let related = client
        .call_tool(
            CallToolRequestParams::new("get_related").with_arguments(rmcp::object!({
                "label": "Hermione",
                "depth": 1,
            })),
        )
        .await
        .expect("call get_related");
    let related_text: String = related
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        related_text.contains("Hogwarts"),
        "get_related should expand: {related_text}"
    );

    // --- Phase 3: hybrid recall ------------------------------------------
    let recall = client
        .call_tool(
            CallToolRequestParams::new("recall_memory").with_arguments(rmcp::object!({
                "query": "where does Hermione study",
                "top_k": 3,
            })),
        )
        .await
        .expect("call recall_memory");
    let recall_text: String = recall
        .content
        .iter()
        .filter_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        recall_text.contains("Relevant memories") && recall_text.contains("Hermione"),
        "recall should return a context block: {recall_text}"
    );

    // Cleanup.
    client
        .call_tool(
            CallToolRequestParams::new("delete_note").with_arguments(rmcp::object!({
                "id": note_id,
            })),
        )
        .await
        .expect("call delete_note");
}
