//! Claim persistence and contradiction handling (V2).
//!
//! A claim is the durable, evidence-backed unit of memory. Claims project to
//! graph `relations` (the canonical projection) via `claim_relations`; each
//! claim retains its evidence span, modality, confidence, status, and the
//! LLM-proposed labels. Contradicting claims (same triple, opposing
//! modality) are linked in `claim_contradictions` and both marked
//! `contradicted` — nothing is silently overwritten.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Claim, ClaimView, ContradictionView};

/// Modalities that oppose each other (true contradiction).
pub fn modalities_oppose(a: &str, b: &str) -> bool {
    (a == "asserted" && b == "negated") || (a == "negated" && b == "asserted")
}

pub struct NewClaim<'a> {
    pub note_id: Uuid,
    pub subject_id: Uuid,
    pub proposed_subject_label: &'a str,
    pub predicate: &'a str,
    pub object_id: Uuid,
    pub proposed_object_label: &'a str,
    pub modality: &'a str,
    pub confidence: f32,
    pub status: &'a str,
    pub evidence_span: Option<String>,
    pub evidence_offset: Option<i32>,
    pub extraction_version: &'a str,
    pub source: &'a str,
    pub source_document_id: Option<Uuid>,
    pub metadata: serde_json::Value,
}

pub async fn insert_claim(pool: &PgPool, c: &NewClaim<'_>) -> Result<Claim, sqlx::Error> {
    sqlx::query_as::<_, Claim>(
        r#"
        INSERT INTO claims (
            note_id, subject_id, proposed_subject_label, predicate, object_id,
            proposed_object_label, modality, confidence, status, evidence_span,
            evidence_offset, extraction_version, source, source_document_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING *
        "#,
    )
    .bind(c.note_id)
    .bind(c.subject_id)
    .bind(c.proposed_subject_label)
    .bind(c.predicate)
    .bind(c.object_id)
    .bind(c.proposed_object_label)
    .bind(c.modality)
    .bind(c.confidence)
    .bind(c.status)
    .bind(&c.evidence_span)
    .bind(c.evidence_offset)
    .bind(c.extraction_version)
    .bind(c.source)
    .bind(c.source_document_id)
    .bind(&c.metadata)
    .fetch_one(pool)
    .await
}

/// A claim already recorded for this exact triple within the same note
/// (same-note dedup; cross-note repeats are corroboration and stay separate).
pub async fn find_claim_by_triple_in_note(
    pool: &PgPool,
    note_id: Uuid,
    subject_id: Uuid,
    predicate: &str,
    object_id: Uuid,
) -> Result<Option<Claim>, sqlx::Error> {
    sqlx::query_as::<_, Claim>(
        r#"
        SELECT * FROM claims
        WHERE note_id = $1 AND subject_id = $2 AND predicate = $3 AND object_id = $4
        LIMIT 1
        "#,
    )
    .bind(note_id)
    .bind(subject_id)
    .bind(predicate)
    .bind(object_id)
    .fetch_optional(pool)
    .await
}

/// Existing claims on the same triple with an opposing modality.
pub async fn find_contradicting_claims(
    pool: &PgPool,
    subject_id: Uuid,
    predicate: &str,
    object_id: Uuid,
    modality: &str,
) -> Result<Vec<Claim>, sqlx::Error> {
    sqlx::query_as::<_, Claim>(
        r#"
        SELECT * FROM claims
        WHERE subject_id = $1 AND predicate = $2 AND object_id = $3
          AND modality <> $4
          AND status <> 'rejected'
        "#,
    )
    .bind(subject_id)
    .bind(predicate)
    .bind(object_id)
    .bind(modality)
    .fetch_all(pool)
    .await
}

pub async fn set_claim_status(pool: &PgPool, claim_id: Uuid, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE claims SET status = $1, updated_at = now() WHERE id = $2")
        .bind(status)
        .bind(claim_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn link_claim_relation(
    pool: &PgPool,
    claim_id: Uuid,
    relation_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO claim_relations (claim_id, relation_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(claim_id)
    .bind(relation_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Link two claims as contradicting each other (canonical order enforced).
pub async fn link_contradiction(
    pool: &PgPool,
    claim_a: Uuid,
    claim_b: Uuid,
) -> Result<(), sqlx::Error> {
    let (lo, hi) = if claim_a < claim_b { (claim_a, claim_b) } else { (claim_b, claim_a) };
    sqlx::query(
        "INSERT INTO claim_contradictions (claim_a, claim_b) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(lo)
    .bind(hi)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

const CLAIM_VIEW_SQL: &str = r#"
    SELECT c.id,
           e1.label AS subject_label,
           c.predicate,
           e2.label AS object_label,
           c.modality, c.confidence, c.status,
           c.evidence_span, c.evidence_offset, c.extraction_version,
           n.content AS note_content,
           c.created_at
    FROM claims c
    JOIN entities e1 ON e1.id = c.subject_id
    JOIN entities e2 ON e2.id = c.object_id
    JOIN notes n ON n.id = c.note_id
"#;

pub async fn get_claim(pool: &PgPool, id: Uuid) -> Result<Option<ClaimView>, sqlx::Error> {
    sqlx::query_as::<_, ClaimView>(&format!("{CLAIM_VIEW_SQL} WHERE c.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn claims_for_note(pool: &PgPool, note_id: Uuid) -> Result<Vec<ClaimView>, sqlx::Error> {
    sqlx::query_as::<_, ClaimView>(&format!(
        "{CLAIM_VIEW_SQL} WHERE c.note_id = $1 ORDER BY c.created_at"
    ))
    .bind(note_id)
    .fetch_all(pool)
    .await
}

pub async fn claims_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<ClaimView>, sqlx::Error> {
    let limit = limit.clamp(1, 200);
    sqlx::query_as::<_, ClaimView>(&format!(
        "{CLAIM_VIEW_SQL} WHERE (c.subject_id = $1 OR c.object_id = $1)
         AND ($2::text IS NULL OR c.status = $2)
         ORDER BY c.created_at DESC LIMIT $3"
    ))
    .bind(entity_id)
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn contradictions_for_claim(
    pool: &PgPool,
    claim_id: Uuid,
) -> Result<Vec<ContradictionView>, sqlx::Error> {
    sqlx::query_as::<_, ContradictionView>(
        r#"
        SELECT cc.claim_a, cc.claim_b,
               e1.label AS subject_label, ca.predicate, e2.label AS object_label,
               ca.modality AS modality_a, cb.modality AS modality_b,
               cc.detected_by, cc.created_at
        FROM claim_contradictions cc
        JOIN claims ca ON ca.id = cc.claim_a
        JOIN claims cb ON cb.id = cc.claim_b
        JOIN entities e1 ON e1.id = ca.subject_id
        JOIN entities e2 ON e2.id = ca.object_id
        WHERE cc.claim_a = $1 OR cc.claim_b = $1
        "#,
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await
}

/// How many claims reference an entity (for observability).
#[allow(dead_code)]
pub async fn count_claims(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM claims")
        .fetch_one(pool)
        .await?;
    Ok(count)
}