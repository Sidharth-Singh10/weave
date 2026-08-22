//! Integration tests for the weave-mcp storage pipeline and MCP protocol.
//!
//! DB-backed tests are gated on a reachable Postgres (`DATABASE_URL` or
//! `WEAVE_MCP_DATABASE_URL`); the target database is `weave_mcp` (created by
//! `db::ensure_and_migrate`). The LLM is forced into deterministic mock mode
//! so tests never hit the network.

use std::sync::Arc;

use weave_core::llm::OpenCodeClient;
use weave_mcp::{claims, db, embed, ingest, retrieval, store};

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
        true,
        None,
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
        true,
        None,
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

/// V2: ingest stores evidence-backed claims; contradiction handling and
/// alias resolution work on the persisted graph.
#[tokio::test]
async fn claims_evidence_contradiction_and_alias() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let llm = Arc::new(OpenCodeClient::mock());
    let embedder = stub_embedder();

    // Ingest an asserted claim; mock extractor handles "X has a Y." via the
    // "has" pattern.
    let note = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Hogwarts has a secret chamber.",
        "note",
        &[],
        "user",
        None,
        true,
        None,
        None,
    )
    .await
    .expect("ingest asserted claim");
    assert!(note.claims_added >= 1, "asserted claim stored");

    let hogwarts = store::find_entity_by_normalized(&pool, &store::normalize_label("Hogwarts"))
        .await
        .unwrap()
        .expect("hogwarts entity");
    let views = claims::claims_for_entity(&pool, hogwarts.id, None, 50).await.unwrap();
    let asserted = views
        .iter()
        .find(|c| c.predicate == "has" && c.status == "active")
        .expect("asserted has-claim from ingest");

    // Evidence + provenance + version recorded.
    let full = claims::get_claim(&pool, asserted.id).await.unwrap().unwrap();
    assert!(full.evidence_span.is_some(), "evidence span recorded");
    assert!(full.note_content.contains("Hogwarts"), "source note linked");
    assert_eq!(full.modality, "asserted");
    assert_eq!(full.confidence, 1.0);
    assert!(!full.extraction_version.is_empty(), "extractor version recorded");

    // A directly-inserted negated claim on the same triple contradicts it.
    let chamber = store::find_entity_by_normalized(&pool, &store::normalize_label("secret chamber"))
        .await
        .unwrap()
        .expect("secret chamber entity");
    let negated = claims::insert_claim(
        &pool,
        &claims::NewClaim {
            note_id: note.note_id,
            subject_id: hogwarts.id,
            proposed_subject_label: "Hogwarts",
            predicate: "has",
            object_id: chamber.id,
            proposed_object_label: "secret chamber",
            modality: "negated",
            confidence: 0.8,
            status: "active",
            evidence_span: Some("Hogwarts has no secret chamber.".to_string()),
            evidence_offset: Some(0),
            extraction_version: "1",
            source: "user",
            source_document_id: None,
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    let opponents = claims::find_contradicting_claims(
        &pool,
        hogwarts.id,
        "has",
        chamber.id,
        "negated",
    )
    .await
    .unwrap();
    assert!(
        opponents.iter().any(|c| c.id == asserted.id),
        "the asserted claim must oppose the negated one"
    );
    claims::set_claim_status(&pool, negated.id, "contradicted").await.unwrap();
    claims::set_claim_status(&pool, asserted.id, "contradicted").await.unwrap();
    claims::link_contradiction(&pool, negated.id, asserted.id).await.unwrap();

    let contradictions = claims::contradictions_for_claim(&pool, asserted.id)
        .await
        .unwrap();
    assert_eq!(contradictions.len(), 1, "contradiction junction recorded");
    assert!(
        claims::modalities_oppose(&contradictions[0].modality_a, &contradictions[0].modality_b),
        "modalities must oppose"
    );

    // Both claims are retrievable with status contradicted (nothing lost).
    let after = claims::claims_for_entity(&pool, hogwarts.id, Some("contradicted"), 50)
        .await
        .unwrap();
    assert!(
        after.len() >= 2,
        "both contradicting claims remain addressable: {after:?}"
    );

    // Alias resolution: reference an entity by a stored alias.
    let rust = match store::find_entity_by_normalized(&pool, &store::normalize_label("Rust"))
        .await
        .unwrap()
    {
        Some(e) => e,
        None => store::insert_entity(&pool, "Rust", "concept").await.unwrap(),
    };
    sqlx::query(
        "UPDATE entities SET aliases = array_append(aliases, 'The Rust Lang') WHERE id = $1",
    )
    .bind(rust.id)
    .execute(&pool)
    .await
    .unwrap();

    let alias_note = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "The Rust Lang is a systems programming language.",
        "note",
        &[],
        "user",
        None,
        true,
        None,
        None,
    )
    .await
    .expect("ingest via alias");
    assert!(alias_note.total_entities >= 1);

    // Cleanup notes (claims cascade with them).
    store::delete_note(&pool, note.note_id).await.unwrap();
}

/// V3: hybrid retrieval with a stub embedder skips the semantic layer,
/// anchors are lexical + 1-hop graph expansion with explainable reasons, and
/// reindex stamps the embedding model.
#[tokio::test]
async fn retrieval_reasons_and_reindex_v3() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let llm = Arc::new(OpenCodeClient::mock());
    let embedder = stub_embedder();

    let note = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Hogwarts has a secret chamber.",
        "note",
        &[],
        "user",
        None,
        true,
        None,
        None,
    )
    .await
    .expect("ingest");
    assert!(!note.retrieval.is_empty(), "ingest returns candidate anchors");
    assert!(
        note.retrieval.iter().all(|c| c.reasons.iter().all(|r| r != "semantic")),
        "stub embedder must not use the semantic layer"
    );

    // Stub embedder: the semantic layer is skipped entirely.
    let candidates = retrieval::retrieve_entities(
        &pool,
        &embedder,
        "Hogwarts has a secret chamber.",
        10,
    )
    .await
    .unwrap();
    assert!(!candidates.is_empty());
    assert!(
        candidates.iter().all(|c| c.reasons.iter().all(|r| r != "semantic")),
        "no semantic reasons with stub embedder"
    );

    // Graph expansion: querying the leaf "secret chamber" surfaces the
    // 1-hop neighbor "Hogwarts".
    let expanded = retrieval::retrieve_entities(&pool, &embedder, "secret chamber secrets", 10)
        .await
        .unwrap();
    let hogwarts = expanded.iter().find(|c| c.entity.label == "Hogwarts");
    assert!(
        hogwarts.is_some(),
        "1-hop neighbor Hogwarts must be retrievable: {expanded:?}"
    );
    assert!(
        hogwarts.unwrap().reasons.iter().any(|r| r.starts_with("1-hop of")),
        "reason must explain the graph expansion"
    );

    // Note embedding is stamped with the current model id.
    let model: Option<String> = sqlx::query_scalar(
        "SELECT embedding_model FROM notes WHERE id = $1",
    )
    .bind(note.note_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(model.as_deref(), Some(embed::STUB_MODEL_ID));

    // Reindex is a no-op for rows already stamped with the current model.
    let result = retrieval::reindex_embeddings(&pool, &embedder, 200).await.unwrap();
    assert_eq!(result.model, embed::STUB_MODEL_ID);
    assert!(!result.semantic);

    store::delete_note(&pool, note.note_id).await.unwrap();
}

/// V4: with the LLM unavailable (mock), the selective verifier falls back to
/// deterministic behavior — no claims are verified, and the risk policy still
/// quarantines unsupported claims.
#[tokio::test]
async fn verifier_falls_back_without_llm_v4() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let llm = Arc::new(OpenCodeClient::mock());
    let embedder = stub_embedder();

    // Mock LLM has no API key -> verify_claim is never invoked.
    let note = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Rust improves compile times and reduces bugs.",
        "note",
        &[],
        "user",
        None,
        true,
        None,
        None,
    )
    .await
    .expect("ingest");
    assert_eq!(
        note.claims_verified, 0,
        "verifier must be skipped when the LLM is unavailable"
    );

    // Unsupported claim is still quarantined deterministically.
    let note2 = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Something about the weather today.",
        "note",
        &[],
        "user",
        None,
        true,
        None,
        None,
    )
    .await
    .expect("ingest unsupported");

    store::delete_note(&pool, note.note_id).await.unwrap();
    store::delete_note(&pool, note2.note_id).await.unwrap();
}

/// V5: idempotent writes return the same note, audit rows are recorded, and
/// supersession works.
#[tokio::test]
async fn idempotency_audit_and_supersession_v5() {
    let _guard = DB_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(pool) = pool().await else {
        eprintln!("skipping: no reachable database");
        return;
    };
    let llm = Arc::new(OpenCodeClient::mock());
    let embedder = stub_embedder();

    let first = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Hogwarts has four houses.",
        "note",
        &[],
        "user",
        None,
        true,
        Some("agent-42"),
        Some(serde_json::json!({ "origin": "test" })),
    )
    .await
    .expect("ingest");
    assert!(!first.duplicate);
    assert!(!first.claim_ids.is_empty(), "write receipt includes claim ids");

    // Re-ingesting identical content returns the same note id.
    let again = ingest::ingest_note(
        &pool,
        &llm,
        &embedder,
        "Hogwarts has four houses.",
        "note",
        &[],
        "user",
        None,
        true,
        Some("agent-42"),
        None,
    )
    .await
    .expect("re-ingest");
    assert!(again.duplicate, "re-ingest must be flagged duplicate");
    assert_eq!(again.note_id, first.note_id, "same note id");
    assert_eq!(again.claim_ids, first.claim_ids, "same claim receipt");

    // Agent attribution + metadata merged onto the note.
    let note = store::get_note(&pool, first.note_id).await.unwrap().unwrap();
    assert_eq!(note.metadata["agent"], "agent-42");
    assert_eq!(note.metadata["origin"], "test");

    // Audit rows were written for note + claim creation by the acting agent.
    let created: Vec<(String, String)> = sqlx::query_as(
        "SELECT action, actor FROM audit_log WHERE target_id = $1 AND action IN ('note.created','claim.created')",
    )
    .bind(first.note_id.to_string())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        created.iter().any(|(a, _)| a == "note.created"),
        "audit must record note creation: {created:?}"
    );
    assert!(
        created.iter().any(|(a, _)| a == "claim.created"),
        "audit must record claim creation: {created:?}"
    );
    assert!(
        created.iter().all(|(_, actor)| actor == "agent-42"),
        "audit actor is the agent: {created:?}"
    );

    // Supersession: a corrected claim marks the original superseded.
    let claim_row = crate::claims::get_claim_row(&pool, first.claim_ids[0])
        .await
        .unwrap()
        .expect("claim row");
    let corrected = crate::claims::insert_claim(
        &pool,
        &crate::claims::NewClaim {
            note_id: claim_row.note_id,
            subject_id: claim_row.subject_id,
            proposed_subject_label: &claim_row.proposed_subject_label,
            predicate: "contains",
            object_id: claim_row.object_id,
            proposed_object_label: &claim_row.proposed_object_label,
            modality: &claim_row.modality,
            confidence: 1.0,
            status: "active",
            evidence_span: claim_row.evidence_span.clone(),
            evidence_offset: claim_row.evidence_offset,
            extraction_version: &claim_row.extraction_version,
            source: "user",
            source_document_id: None,
            metadata: serde_json::json!({ "corrected": true, "supersedes": claim_row.id }),
        },
    )
    .await
    .unwrap();
    crate::claims::supersede_claim(&pool, claim_row.id, corrected.id)
        .await
        .unwrap();

    let superseded = crate::claims::get_claim_row(&pool, claim_row.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(superseded.status, "superseded");
    assert_eq!(superseded.metadata["superseded_by"], corrected.id.to_string());

    // Stats reflect the counts.
    let by_status = store::count_claims_by_status(&pool).await.unwrap();
    assert!(
        by_status.iter().any(|(s, c)| s == "superseded" && *c >= 1),
        "superseded claims counted: {by_status:?}"
    );

    store::delete_note(&pool, first.note_id).await.unwrap();
}
