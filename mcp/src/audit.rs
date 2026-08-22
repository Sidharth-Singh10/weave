//! Audit log (V5): durable, queryable record of significant memory mutations.
//!
//! Every write that changes memory (note created, entity created, claim
//! verdicts, contradiction links, alias additions, corrections, deletions)
//! is recorded here with the acting agent, the action, and before/after
//! values. Recording is best-effort — it never fails the operation.

use serde_json::{Value, json};
use sqlx::PgPool;

/// Actions recorded in the audit log.
pub mod action {
    pub const NOTE_CREATED: &str = "note.created";
    pub const NOTE_DELETED: &str = "note.deleted";
    pub const ENTITY_CREATED: &str = "entity.created";
    pub const ENTITY_DELETED: &str = "entity.deleted";
    pub const ALIAS_ADDED: &str = "entity.alias_added";
    pub const CLAIM_CREATED: &str = "claim.created";
    pub const CLAIM_VERIFIED: &str = "claim.verified";
    pub const CLAIM_REJECTED: &str = "claim.rejected";
    pub const CLAIM_QUARANTINED: &str = "claim.quarantined";
    pub const CLAIM_SUPERSEDED: &str = "claim.superseded";
    pub const CONTRADICTION_LINKED: &str = "claim.contradiction_linked";
    pub const EMBEDDINGS_REINDEXED: &str = "embeddings.reindexed";
}

/// Record one audit entry. Best-effort (errors are logged, never returned).
pub async fn record(
    pool: &PgPool,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    old_value: Option<Value>,
    new_value: Option<Value>,
) {
    let res = sqlx::query(
        "INSERT INTO audit_log (actor, action, target_type, target_id, old_value, new_value)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await;
    if let Err(e) = res {
        tracing::warn!(error = %e, action, "audit record failed");
    }
}

/// Convenience: record with only a new value.
pub async fn record_new(
    pool: &PgPool,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    new_value: Value,
) {
    record(pool, actor, action, target_type, target_id, None, Some(new_value)).await;
}

/// Convenience: record a generic one-shot action (no target).
pub async fn record_event(pool: &PgPool, actor: &str, action: &str) {
    record(pool, actor, action, None, None, None, Some(json!({}))).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_constants_are_stable() {
        assert_eq!(action::NOTE_CREATED, "note.created");
        assert_eq!(action::CLAIM_SUPERSEDED, "claim.superseded");
    }
}