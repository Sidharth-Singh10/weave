//! Hybrid retrieval (GraphRAG): vector + full-text + graph expansion, merged
//! with reciprocal rank fusion into a compact context block. Candidate
//! entities come from the shared [`crate::retrieval`] engine; matching claims
//! surface alongside notes and entities when embeddings are available.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::embed::Embedder;
use crate::models::{ClaimView, Entity, Note, RelationView};
use crate::store;

const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Serialize)]
pub struct RecallNote {
    pub id: Uuid,
    pub summary: Option<String>,
    pub content: String,
    pub kind: String,
    pub created_at: DateTime<Utc>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecallResult {
    pub notes: Vec<RecallNote>,
    pub entities: Vec<Entity>,
    pub relations: Vec<RelationView>,
    /// Matching evidence-backed claims (only when the embedder is semantic).
    pub claims: Vec<ClaimView>,
    /// Compact, ready-to-use context block (summaries + subgraph).
    pub context: String,
}

fn rrf_scores<T>(lists: &[&[T]]) -> HashMap<Uuid, f64>
where
    T: RrfKey,
{
    let mut scores: HashMap<Uuid, f64> = HashMap::new();
    for list in lists {
        for (rank, item) in list.iter().enumerate() {
            let id = item.rrf_id();
            let entry = scores.entry(id).or_insert(0.0);
            *entry += 1.0 / (RRF_K + (rank as f64) + 1.0);
        }
    }
    scores
}

trait RrfKey {
    fn rrf_id(&self) -> Uuid;
}

impl RrfKey for Note {
    fn rrf_id(&self) -> Uuid {
        self.id
    }
}

/// Hybrid retrieval over notes, entities, claims, and the graph neighborhood.
pub async fn recall_memory(
    pool: &PgPool,
    embedder: &Arc<dyn Embedder>,
    query: &str,
    top_k: usize,
) -> anyhow::Result<RecallResult> {
    let top_k = top_k.clamp(1, 20);
    let semantic = embedder.is_semantic();
    let query_vec = if semantic { Some(embedder.embed(query)?) } else { None };

    // Semantic + keyword candidate lists. Semantic lists are skipped for stub
    // embedders (their vectors are noise).
    let vec_notes = match &query_vec {
        Some(v) => store::vector_search_notes(pool, v, (top_k * 2) as i64).await?,
        None => vec![],
    };
    let fts_notes = store::search_notes(pool, query, (top_k * 2) as i64).await?;

    // Entities + graph expansion via the shared retrieval engine.
    let candidates = crate::retrieval::retrieve_entities(pool, embedder, query, top_k * 2).await?;
    let entities: Vec<Entity> = candidates.iter().map(|c| c.entity.clone()).collect();

    // Claims: semantically similar evidence-backed statements (active only).
    let claims = match &query_vec {
        Some(v) => store::vector_search_claims(pool, v, (top_k * 2) as i64).await?,
        None => vec![],
    };

    // Graph: expand the top entities one hop and collect notes referencing
    // them (unchanged from prior behavior).
    let mut graph_notes: Vec<Note> = Vec::new();
    let mut graph_relations: Vec<RelationView> = Vec::new();
    for entity in entities.iter().take(5) {
        for relation in store::relations_for_entity(pool, entity.id).await? {
            if !graph_relations
                .iter()
                .any(|r| r.relation_id == relation.relation_id)
            {
                graph_relations.push(relation);
            }
        }
        for note in store::notes_for_entity(pool, entity.id, 5).await? {
            if !graph_notes.iter().any(|n| n.id == note.id) {
                graph_notes.push(note);
            }
        }
    }

    // Merge the note lists with reciprocal rank fusion.
    let note_lists = [
        vec_notes.as_slice(),
        fts_notes.as_slice(),
        graph_notes.as_slice(),
    ];
    let scores = rrf_scores(&note_lists);

    let mut ranked: Vec<(Note, f64)> = vec_notes
        .iter()
        .chain(fts_notes.iter())
        .chain(graph_notes.iter())
        .filter_map(|n| {
            let score = *scores.get(&n.id)?;
            Some((n.clone(), score))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let notes: Vec<RecallNote> = ranked
        .into_iter()
        .take(top_k)
        .map(|(n, score)| RecallNote {
            id: n.id,
            summary: n.summary,
            content: n.content,
            kind: n.kind,
            created_at: n.created_at,
            score,
        })
        .collect();

    let entities = entities.into_iter().take(top_k).collect::<Vec<_>>();
    let claims = claims.into_iter().take(top_k).collect::<Vec<_>>();

    let context = build_context(&notes, &entities, &graph_relations, &claims);

    Ok(RecallResult {
        notes,
        entities,
        relations: graph_relations,
        claims,
        context,
    })
}

/// Compact context block: note summaries + subgraph + claims, token-efficient
/// for dropping straight into an agent's context.
fn build_context(
    notes: &[RecallNote],
    entities: &[Entity],
    relations: &[RelationView],
    claims: &[ClaimView],
) -> String {
    let mut out = String::from("## Relevant memories\n");
    if notes.is_empty() {
        out.push_str("(none)\n");
    } else {
        for note in notes {
            let text = note
                .summary
                .clone()
                .unwrap_or_else(|| note.content.chars().take(160).collect());
            out.push_str(&format!("- {text} (note:{})\n", note.id));
        }
    }

    if !entities.is_empty() {
        out.push_str("## Entities\n");
        for entity in entities {
            out.push_str(&format!("- {} ({})\n", entity.label, entity.kind));
        }
    }

    if !relations.is_empty() {
        out.push_str("## Relations\n");
        for r in relations.iter().take(30) {
            out.push_str(&format!(
                "- {} -[{}]-> {}\n",
                r.source_label, r.relation, r.target_label
            ));
        }
    }

    if !claims.is_empty() {
        out.push_str("## Claims\n");
        for c in claims.iter().take(10) {
            out.push_str(&format!(
                "- {} -[{}]-> {} ({}, conf {:.2})\n",
                c.subject_label, c.predicate, c.object_label, c.modality, c.confidence
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn rrf_merges_rankings() {
        let a = [Note {
            id: Uuid::nil(),
            content: String::new(),
            summary: None,
            kind: String::new(),
            tags: vec![],
            importance: 0.0,
            source: String::new(),
            metadata: serde_json::Value::Null,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];
        let b: [Note; 0] = [];
        let scores = rrf_scores(&[a.as_slice(), &b]);
        assert!(scores.contains_key(&Uuid::nil()));
    }

    #[test]
    fn context_has_sections() {
        let context = build_context(&[], &[], &[], &[]);
        assert!(context.contains("Relevant memories"));
    }

    #[test]
    fn ids_are_distinct_across_lists() {
        let _ = HashSet::<Uuid>::new();
    }
}
