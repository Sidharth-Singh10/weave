//! Note/document ingestion: extract claims, resolve entities, persist with
//! provenance (V2).
//!
//! "LLM proposes, system verifies": the extractor emits free-form nodes and
//! edges; this module validates every candidate, resolves entities
//! deterministically (exact -> alias -> create), infers modality/evidence
//! from the source note, and commits only supported claims. Unsupported
//! claims are quarantined; contradicting claims are linked and both marked
//! `contradicted`.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use weave_core::extract;
use weave_core::llm::OpenCodeClient;
use weave_core::models::{GraphNode, IngestRequest};

use crate::claims;
use crate::embed::Embedder;
use crate::models::{Entity, IngestResult};
use crate::retrieval::RetrievalCandidate;
use crate::store;
use crate::{entity, retrieval, validate};

/// Ingest a note: summarize, persist the note, extract + validate claims,
/// resolve entities, project relations, record provenance, and embed the note
/// and new entities.
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
        let _ = store::set_note_embedding(pool, note.id, &embedding, embedder.model_id()).await;
    }

    // Candidate anchors: hybrid lexical + semantic retrieval (the semantic
    // layer is skipped for stub embedders). Passed to the LLM as context so it
    // reuses existing labels.
    let retrieval: Vec<RetrievalCandidate> =
        retrieval::retrieve_entities(pool, embedder, content, 50).await?;
    let existing: Vec<Entity> = retrieval.iter().map(|c| c.entity.clone()).collect();
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

    let extraction_version = extract::EXTRACTOR_VERSION;
    let mut entities_used: Vec<Entity> = existing.clone();
    let mut resolved: HashMap<String, entity::ResolvedEntity> = HashMap::new();

    // Anchor entities found in the text are already resolved; seed the map so
    // edge endpoints reuse them even when the extractor emits no new nodes.
    for e in &existing {
        resolved.insert(
            store::normalize_label(&e.label),
            entity::ResolvedEntity {
                entity: e.clone(),
                created: false,
                method: entity::ResolutionMethod::Exact,
                ambiguous_candidates: vec![],
            },
        );
    }

    let mut entities_added: Vec<String> = Vec::new();
    let mut relations_added: Vec<String> = Vec::new();
    let mut claims_added = 0i64;
    let mut claims_quarantined = 0i64;
    let mut claims_rejected = 0i64;
    let mut contradictions_detected = 0i64;

    // Pass 1: validate every proposed node label; resolve valid ones.
    // Kinds come from the LLM nodes so edge endpoints reuse them.
    let mut kind_for_label: HashMap<String, String> = HashMap::new();
    for node in &delta.nodes {
        let key = store::normalize_label(&node.label);
        kind_for_label
            .entry(key)
            .or_insert_with(|| node.kind.clone());
    }

    for node in &delta.nodes {
        if let Err(reason) = validate::validate_label(&node.label) {
            tracing::debug!(label = %node.label, reason, "entity label rejected");
            continue;
        }
        let key = store::normalize_label(&node.label);
        if resolved.contains_key(&key) {
            continue;
        }
        let r = entity::resolve(pool, &node.label, &node.kind).await?;
        if r.created {
            entities_added.push(node.label.clone());
            if let Ok(embedding) = embedder.embed(&node.label) {
                let _ =
                    store::set_entity_embedding(pool, r.entity.id, &embedding, embedder.model_id())
                        .await;
            }
        }
        if !entities_used.iter().any(|e| e.id == r.entity.id) {
            entities_used.push(r.entity.clone());
        }
        resolved.insert(key, r);
    }

    // Pass 2: validate each edge as a claim; resolve endpoints; persist.
    for edge in &delta.edges {
        let candidate = match validate::validate_claim(
            content,
            &edge.source_label,
            &edge.relation,
            &edge.target_label,
        ) {
            Ok(c) => c,
            Err(reason) => {
                tracing::debug!(reason, "claim rejected");
                claims_rejected += 1;
                continue;
            }
        };

        let skey = store::normalize_label(&candidate.subject_label);
        let okey = store::normalize_label(&candidate.object_label);
        let (Some(subject_r), Some(object_r)) = (resolved.get(&skey), resolved.get(&okey)) else {
            claims_rejected += 1;
            continue;
        };
        let subject = &subject_r.entity;
        let object = &object_r.entity;

        // Same-note dedup: an identical triple already recorded for this note.
        if let Some(existing) = claims::find_claim_by_triple_in_note(
            pool,
            note.id,
            subject.id,
            &candidate.predicate,
            object.id,
        )
        .await?
        {
            if let Some(relation) = store::find_relation(pool, subject.id, object.id, &candidate.predicate)
                .await?
            {
                claims::link_claim_relation(pool, existing.id, relation.id).await?;
                store::link_note_relation(pool, note.id, relation.id).await?;
            }
            continue;
        }

        let status = if candidate.supported { "active" } else { "quarantined" };
        let confidence = if candidate.supported {
            candidate.confidence
        } else {
            0.3
        };
        let resolution = json!({
            "subject": {
                "method": subject_r.method.as_str(),
                "ambiguous_candidates": subject_r.ambiguous_candidates,
            },
            "object": {
                "method": object_r.method.as_str(),
                "ambiguous_candidates": object_r.ambiguous_candidates,
            },
        });
        let claim = claims::insert_claim(
            pool,
            &claims::NewClaim {
                note_id: note.id,
                subject_id: subject.id,
                proposed_subject_label: &candidate.subject_label,
                predicate: &candidate.predicate,
                object_id: object.id,
                proposed_object_label: &candidate.object_label,
                modality: &candidate.modality,
                confidence,
                status,
                evidence_span: candidate.evidence_span.clone(),
                evidence_offset: candidate.evidence_offset,
                extraction_version,
                source,
                source_document_id,
                metadata: serde_json::json!({ "entity_resolution": resolution }),
            },
        )
        .await?;
        if status == "quarantined" {
            claims_quarantined += 1;
        } else {
            claims_added += 1;
        }

        // Embed the claim (subject predicate object) so recall can surface it.
        if let Ok(embedding) = embedder.embed(&retrieval::claim_embedding_text(
            &candidate.subject_label,
            &candidate.predicate,
            &candidate.object_label,
        )) {
            let _ =
                store::set_claim_embedding(pool, claim.id, &embedding, embedder.model_id()).await;
        }

        // Project to the graph relation (canonical unique source/target/rel).
        let relation = match store::insert_relation(pool, subject.id, object.id, &candidate.predicate)
            .await?
        {
            Some(relation) => {
                relations_added.push(format!(
                    "{} -[{}]-> {}",
                    subject.label, candidate.predicate, object.label
                ));
                relation
            }
            None => store::find_relation(pool, subject.id, object.id, &candidate.predicate)
                .await?
                .expect("relation exists after conflict"),
        };
        claims::link_claim_relation(pool, claim.id, relation.id).await?;
        store::link_note_relation(pool, note.id, relation.id).await?;

        // Contradiction detection: same triple, opposing modality.
        let opponents = claims::find_contradicting_claims(
            pool,
            subject.id,
            &candidate.predicate,
            object.id,
            &candidate.modality,
        )
        .await?;
        for opp in opponents {
            if opp.id == claim.id {
                continue;
            }
            claims::set_claim_status(pool, claim.id, "contradicted").await?;
            claims::set_claim_status(pool, opp.id, "contradicted").await?;
            claims::link_contradiction(pool, claim.id, opp.id).await?;
            contradictions_detected += 1;
        }

        for endpoint in [subject, object] {
            if !entities_used.iter().any(|e| e.id == endpoint.id) {
                entities_used.push(endpoint.clone());
            }
        }
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
        claims_added,
        claims_quarantined,
        claims_rejected,
        contradictions_detected,
        retrieval,
        total_entities,
        total_relations,
    })
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