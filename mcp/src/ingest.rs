//! Note/document ingestion: extract entities + relations and persist them with
//! provenance.
//!
//! The LLM receives the note plus any existing entities it already mentions so
//! extraction reuses labels; new nodes/edges are resolved against the store
//! (exact normalized match) before creation.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;
use weave_core::extract;
use weave_core::llm::OpenCodeClient;
use weave_core::models::{GraphNode, IngestRequest};

use crate::embed::Embedder;
use crate::models::{Entity, IngestResult};
use crate::store;

/// Ingest a note: summarize, persist the note, extract + resolve entities and
/// relations, record provenance, and embed the note and new entities.
pub async fn ingest_note(
    pool: &PgPool,
    llm: &Arc<OpenCodeClient>,
    embedder: &Arc<dyn Embedder>,
    content: &str,
    kind: &str,
    tags: &[String],
    source: &str,
    source_document_id: Option<Uuid>,
) -> Result<IngestResult, anyhow::Error> {
    if content.trim().is_empty() {
        anyhow::bail!("note content must not be empty");
    }

    let summary = crate::summary::summarize_arc(llm, content).await;
    let note = store::insert_note(
        pool,
        content,
        summary.as_deref(),
        kind,
        tags,
        source,
        source_document_id,
    )
    .await?;

    // Embed the note (summary + content) best-effort.
    let embed_text = match &summary {
        Some(s) => format!("{s}\n{content}"),
        None => content.to_string(),
    };
    if let Ok(embedding) = embedder.embed(&embed_text) {
        let _ = store::set_note_embedding(pool, note.id, &embedding).await;
    }

    // Existing entities the note mentions — passed to the LLM as context so it
    // reuses existing labels.
    let existing = store::find_entities_in_text(pool, content).await?;
    let existing_nodes: Vec<GraphNode> = existing
        .iter()
        .map(|e| GraphNode {
            id: Some(e.id.to_string()),
            label: e.label.clone(),
            kind: e.kind.clone(),
        })
        .collect();

    let req = IngestRequest {
        text: content.to_string(),
        nodes: existing_nodes,
        edges: vec![],
    };
    let (delta, _usage) = extract::extract_delta(llm.as_ref(), &req).await;

    // label (normalized) -> entity, seeded with the existing ones. Every
    // entity the note mentions is linked as provenance.
    let mut entities_used: Vec<Entity> = Vec::new();
    let mut by_label: HashMap<String, Entity> = existing
        .into_iter()
        .map(|e| {
            entities_used.push(e.clone());
            (store::normalize_label(&e.label), e)
        })
        .collect();

    let mut entities_added: Vec<String> = Vec::new();
    let mut relations_added: Vec<String> = Vec::new();

    for node in &delta.nodes {
        let key = store::normalize_label(&node.label);
        let entity = if let Some(existing) = by_label.get(&key) {
            existing.clone()
        } else {
            let created = create_entity(pool, embedder, &node.label, &node.kind).await?;
            entities_added.push(node.label.clone());
            by_label.insert(key.clone(), created.clone());
            created
        };
        if !entities_used.iter().any(|e| e.id == entity.id) {
            entities_used.push(entity);
        }
    }

    for edge in &delta.edges {
        let source_key = store::normalize_label(&edge.source_label);
        let target_key = store::normalize_label(&edge.target_label);

        // Edge endpoints must resolve to entities; create missing ones
        // defensively (should not normally happen).
        let source = if let Some(e) = by_label.get(&source_key) {
            e.clone()
        } else {
            let created = create_entity(pool, embedder, &edge.source_label, "concept").await?;
            entities_added.push(edge.source_label.clone());
            by_label.insert(source_key.clone(), created.clone());
            created
        };
        let target = if let Some(e) = by_label.get(&target_key) {
            e.clone()
        } else {
            let created = create_entity(pool, embedder, &edge.target_label, "concept").await?;
            entities_added.push(edge.target_label.clone());
            by_label.insert(target_key.clone(), created.clone());
            created
        };

        for endpoint in [&source, &target] {
            if !entities_used.iter().any(|e| e.id == endpoint.id) {
                entities_used.push(endpoint.clone());
            }
        }

        // Always link the relation to the note; only count it as "added" when
        // newly created.
        let relation =
            match store::insert_relation(pool, source.id, target.id, &edge.relation).await? {
                Some(relation) => {
                    relations_added.push(format!(
                        "{} -[{}]-> {}",
                        source.label, edge.relation, target.label
                    ));
                    relation
                }
                None => store::find_relation(pool, source.id, target.id, &edge.relation)
                    .await?
                    .expect("relation exists after conflict"),
            };
        store::link_note_relation(pool, note.id, relation.id).await?;
    }

    // Provenance: this note references every resolved entity.
    for entity in &entities_used {
        store::link_note_entity(pool, note.id, entity.id).await?;
    }

    let total_entities = store::count_entities(pool).await?;
    let total_relations = store::count_relations(pool).await?;

    Ok(IngestResult {
        note_id: note.id,
        summary,
        entities_added,
        relations_added,
        total_entities,
        total_relations,
    })
}

/// Create an entity (best-effort embedding of its label).
async fn create_entity(
    pool: &PgPool,
    embedder: &Arc<dyn Embedder>,
    label: &str,
    kind: &str,
) -> Result<Entity, anyhow::Error> {
    let entity = store::get_or_create_entity(pool, label, kind).await?;
    if let Ok(embedding) = embedder.embed(label) {
        let _ = store::set_entity_embedding(pool, entity.id, &embedding).await;
    }
    Ok(entity)
}

/// Ingest the extracted text of a stored document as a `file`-sourced note.
pub async fn ingest_document_text(
    pool: &PgPool,
    llm: &Arc<OpenCodeClient>,
    embedder: &Arc<dyn Embedder>,
    document_id: Uuid,
    text: &str,
) -> Result<IngestResult, anyhow::Error> {
    ingest_note(
        pool,
        llm,
        embedder,
        text,
        "note",
        &[],
        "file",
        Some(document_id),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_case_insensitive() {
        assert_eq!(store::normalize_label("  Harry Potter  "), "harry potter");
    }
}
