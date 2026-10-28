//! Shared hybrid retrieval engine (V3): lexical + semantic candidate
//! retrieval with graph expansion and an explainable score.
//!
//! Used by both write-time grounding (ingest) and read-time recall. The
//! semantic layer is skipped when the embedder is not semantically meaningful
//! (stub), so entity selection is never anchored on random vectors.
//!
//! Score: `0.6·lexical + 0.4·similarity − hub_penalty`, where lexical is
//! mention 0.9 / keyword 0.6 / 1-hop-neighbor 0.4, similarity is cosine
//! (0 when the semantic layer is absent), and hub penalty is
//! `min(0.2, 0.01·ln(1+degree))`. Every candidate records its reasons so a
//! tool can explain why it was selected.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::embed::Embedder;
use crate::models::Entity;
use crate::store;

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalCandidate {
    pub entity: Entity,
    pub score: f64,
    /// Cosine similarity (0..1); 0 when only lexical layers matched.
    pub similarity: f32,
    /// Why this entity was selected, e.g. ["mention", "semantic"].
    pub reasons: Vec<String>,
}

fn hub_penalty(degree: usize) -> f64 {
    let penalty = 0.01 * (1.0 + degree as f64).ln();
    penalty.min(0.2)
}

/// The features that decide one candidate's score.
///
/// Kept separate from the candidate union so the ranking can be evaluated and
/// changed deliberately — see `tests/retrieval_eval.rs`, which measures
/// recall@k and MRR over a fixed corpus and fails when a weight change makes
/// retrieval worse.
#[derive(Debug, Clone, Copy, Default)]
pub struct Signals {
    /// The entity's label or an alias appears verbatim in the query.
    pub mentioned: bool,
    /// The entity matched by keyword, but was not mentioned.
    pub keyword: bool,
    /// Reached as a 1-hop neighbor of a top anchor.
    pub neighbor: bool,
    /// Cosine similarity, when the semantic layer is active.
    pub similarity: Option<f32>,
    /// Incident relation count, used for the hub penalty.
    pub degree: usize,
}

/// The documented ranking formula, in one place.
///
/// Lexical layers are mutually exclusive in priority order
/// (mention > keyword > neighbor). A semantic score blends over whatever base
/// exists.
///
/// Note: the hub penalty is applied twice on blended candidates (once inside
/// the lexical base, once when blending) and once on semantic-only candidates.
/// That asymmetry is preserved on purpose: this function is the baseline the
/// evaluation corpus was measured against, and changing it should show up as a
/// metric change rather than slip in silently.
pub fn score(signals: &Signals) -> f64 {
    let base = if signals.mentioned {
        0.9
    } else if signals.keyword {
        0.6
    } else if signals.neighbor {
        0.4
    } else {
        0.0
    };
    let penalty = hub_penalty(signals.degree);

    match signals.similarity {
        Some(similarity) => {
            if base == 0.0 {
                0.4 * similarity as f64 - penalty
            } else {
                0.6 * (base - penalty) + 0.4 * similarity as f64 - penalty
            }
        }
        None => base - penalty,
    }
}

/// Read the signals back off a candidate's recorded reasons, so a semantic
/// merge can score it exactly as its first insertion would have.
fn signals_from(candidate: &RetrievalCandidate, degree: usize, similarity: f32) -> Signals {
    Signals {
        mentioned: candidate.reasons.iter().any(|r| r == "mention"),
        keyword: candidate.reasons.iter().any(|r| r == "keyword"),
        neighbor: candidate.reasons.iter().any(|r| r.starts_with("1-hop of ")),
        similarity: Some(similarity),
        degree,
    }
}

/// Retrieve the entities most relevant to `query` (a note during ingest, a
/// user query during recall). Candidate union: lexical mention/keyword,
/// semantic similarity, and 1-hop graph neighbors of the top anchors.
pub async fn retrieve_entities(
    pool: &PgPool,
    embedder: &Arc<dyn Embedder>,
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<RetrievalCandidate>> {
    let top_k = top_k.clamp(1, 100);

    let mut by_id: HashMap<Uuid, RetrievalCandidate> = HashMap::new();

    // -- Lexical layer -----------------------------------------------------
    // Entities whose label/alias appears in the text (explicit mention).
    let mentioned = store::find_entities_in_text(pool, query).await?;
    for entity in mentioned {
        let degree = store::relations_for_entity(pool, entity.id).await?.len();
        by_id.insert(
            entity.id,
            RetrievalCandidate {
                entity,
                score: score(&Signals {
                    mentioned: true,
                    degree,
                    ..Signals::default()
                }),
                similarity: 0.0,
                reasons: vec!["mention".to_string()],
            },
        );
    }

    // Keyword matches on label/description.
    let keywords = store::search_entities(pool, query, top_k as i64).await?;
    for entity in keywords {
        let degree = store::relations_for_entity(pool, entity.id).await?.len();
        by_id
            .entry(entity.id)
            .and_modify(|c| {
                if !c.reasons.contains(&"keyword".to_string()) {
                    c.reasons.push("keyword".to_string());
                }
            })
            .or_insert(RetrievalCandidate {
                entity,
                score: score(&Signals {
                    keyword: true,
                    degree,
                    ..Signals::default()
                }),
                similarity: 0.0,
                reasons: vec!["keyword".to_string()],
            });
    }

    // -- Semantic layer (only with a real embedder) ------------------------
    if embedder.is_semantic() {
        let query_vec = embedder.embed(query)?;
        let semantic = store::vector_search_entities_scored(pool, &query_vec, top_k as i64).await?;
        for (entity, similarity) in semantic {
            let degree = store::relations_for_entity(pool, entity.id).await?.len();
            let semantic_only = score(&Signals {
                similarity: Some(similarity),
                degree,
                ..Signals::default()
            });
            by_id
                .entry(entity.id)
                .and_modify(|c| {
                    c.similarity = similarity;
                    c.score = score(&signals_from(c, degree, similarity));
                    c.reasons.push("semantic".to_string());
                })
                .or_insert(RetrievalCandidate {
                    entity,
                    score: semantic_only,
                    similarity,
                    reasons: vec!["semantic".to_string()],
                });
        }
    }

    // -- Graph expansion: 1-hop neighbors of the top anchors ----------------
    let anchors: Vec<Uuid> = {
        let mut ranked: Vec<&RetrievalCandidate> = by_id.values().collect();
        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        ranked.iter().take(5).map(|c| c.entity.id).collect()
    };
    for anchor_id in anchors {
        let relations = store::relations_for_entity(pool, anchor_id).await?;
        for r in relations {
            // The neighbor is the endpoint other than the anchor.
            let neighbor_id = if r.source_id == anchor_id { r.target_id } else { r.source_id };
            if by_id.contains_key(&neighbor_id) {
                continue;
            }
            let entity = match store::get_entity_by_id(pool, neighbor_id).await? {
                Some(e) => e,
                None => continue,
            };
            let degree = store::relations_for_entity(pool, neighbor_id).await?.len();
            by_id.insert(
                neighbor_id,
                RetrievalCandidate {
                    entity,
                    score: score(&Signals {
                        neighbor: true,
                        degree,
                        ..Signals::default()
                    }),
                    similarity: 0.0,
                    reasons: vec![format!("1-hop of {anchor_id}")],
                },
            );
        }
    }

    let mut ranked: Vec<RetrievalCandidate> = by_id.into_values().collect();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(top_k);
    Ok(ranked)
}

/// Embed the "subject predicate object" rendering of a claim.
pub fn claim_embedding_text(subject: &str, predicate: &str, object: &str) -> String {
    format!("{subject} {predicate} {object}")
}

/// Reindex result summary for the `reindex_embeddings` tool.
#[derive(Debug, Clone, Serialize)]
pub struct ReindexResult {
    pub model: &'static str,
    pub semantic: bool,
    pub notes_reindexed: i64,
    pub entities_reindexed: i64,
    pub claims_reindexed: i64,
}

/// Re-embed rows whose `embedding_model` does not match the current embedder
/// (stub/no vectors or a stale model). On-demand, best-effort.
pub async fn reindex_embeddings(
    pool: &PgPool,
    embedder: &Arc<dyn Embedder>,
    limit: i64,
) -> anyhow::Result<ReindexResult> {
    let model = embedder.model_id();
    let notes = store::notes_needing_reindex(pool, model, limit).await?;
    let entities = store::entities_needing_reindex(pool, model, limit).await?;
    let claims = store::claims_needing_reindex(pool, model, limit).await?;

    let mut notes_count = 0i64;
    let mut entities_count = 0i64;
    let mut claims_count = 0i64;

    for note in notes {
        let text = note
            .summary
            .clone()
            .map(|s| format!("{s}\n{}", note.content))
            .unwrap_or_else(|| note.content.clone());
        if let Ok(embedding) = embedder.embed(&text) {
            if store::set_note_embedding(pool, note.id, &embedding, model)
                .await
                .is_ok()
            {
                notes_count += 1;
            }
        }
    }
    for entity in entities {
        let text = entity.description.as_deref().map(|d| format!("{} — {d}", entity.label)).unwrap_or_else(|| entity.label.clone());
        if let Ok(embedding) = embedder.embed(&text) {
            if store::set_entity_embedding(pool, entity.id, &embedding, model)
                .await
                .is_ok()
            {
                entities_count += 1;
            }
        }
    }
    for claim in claims {
        let text = claim_embedding_text(
            &claim.proposed_subject_label,
            &claim.predicate,
            &claim.proposed_object_label,
        );
        if let Ok(embedding) = embedder.embed(&text) {
            if store::set_claim_embedding(pool, claim.id, &embedding, model)
                .await
                .is_ok()
            {
                claims_count += 1;
            }
        }
    }

    Ok(ReindexResult {
        model,
        semantic: embedder.is_semantic(),
        notes_reindexed: notes_count,
        entities_reindexed: entities_count,
        claims_reindexed: claims_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_penalty_bounded() {
        assert_eq!(hub_penalty(0), 0.0);
        // Monotonic increasing in degree, capped at 0.2.
        assert!(hub_penalty(100) > hub_penalty(10));
        assert!(hub_penalty(10_000) <= 0.2);
    }

    /// The scorer must reproduce the historical weights exactly: the eval
    /// corpus baseline was measured against these numbers.
    #[test]
    fn score_keeps_documented_weights() {
        let degree = 0;
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;

        assert!(close(
            score(&Signals {
                mentioned: true,
                degree,
                ..Signals::default()
            }),
            0.9
        ));
        assert!(close(
            score(&Signals {
                keyword: true,
                degree,
                ..Signals::default()
            }),
            0.6
        ));
        assert!(close(
            score(&Signals {
                neighbor: true,
                degree,
                ..Signals::default()
            }),
            0.4
        ));
        // Semantic-only: 0.4·similarity − penalty.
        assert!(close(
            score(&Signals {
                similarity: Some(1.0),
                degree,
                ..Signals::default()
            }),
            0.4
        ));
        // Mention blended with semantic: 0.6·0.9 + 0.4·1.0.
        assert!(close(
            score(&Signals {
                mentioned: true,
                similarity: Some(1.0),
                degree,
                ..Signals::default()
            }),
            0.94
        ));
    }

    /// Lexical layers outrank each other at equal degree, regardless of order.
    #[test]
    fn lexical_priority_is_mention_keyword_neighbor() {
        let degree = 5;
        let mention = score(&Signals {
            mentioned: true,
            degree,
            ..Signals::default()
        });
        let keyword = score(&Signals {
            keyword: true,
            degree,
            ..Signals::default()
        });
        let neighbor = score(&Signals {
            neighbor: true,
            degree,
            ..Signals::default()
        });
        assert!(mention > keyword && keyword > neighbor);
    }

    /// Connection count suppresses hubs without inverting lexical priority.
    #[test]
    fn hub_penalty_suppresses_without_inverting() {
        let hub = score(&Signals {
            mentioned: true,
            degree: 1_000,
            ..Signals::default()
        });
        let leaf = score(&Signals {
            mentioned: true,
            degree: 1,
            ..Signals::default()
        });
        assert!(hub < leaf);

        // A huge hub still beats a weak neighbor: the penalty is bounded.
        let weak_neighbor = score(&Signals {
            neighbor: true,
            degree: 1,
            ..Signals::default()
        });
        assert!(hub > weak_neighbor);
    }

    #[test]
    fn claim_text_is_triple() {
        assert_eq!(
            claim_embedding_text("Harry Potter", "studies at", "Hogwarts"),
            "Harry Potter studies at Hogwarts"
        );
    }
}