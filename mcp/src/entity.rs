//! Deterministic entity resolution (V2).
//!
//! Write-path resolution is layered: exact normalized label -> unique alias
//! match -> create. Ambiguous alias matches (more than one candidate)
//! abstain — a new entity is created and the candidate ids are recorded on
//! the claim, never a destructive merge. Existing kinds are never overwritten.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Entity;
use crate::store;

/// How an entity was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionMethod {
    Exact,
    Alias,
    Created,
}

impl ResolutionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionMethod::Exact => "exact",
            ResolutionMethod::Alias => "alias",
            ResolutionMethod::Created => "created",
        }
    }
}

/// A resolved entity plus the audit trail of how it was reached.
#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    pub entity: Entity,
    pub created: bool,
    pub method: ResolutionMethod,
    /// Candidate ids when an alias matched more than one entity (abstain).
    pub ambiguous_candidates: Vec<Uuid>,
}

/// Resolve `label` to an entity: exact normalized match, then a unique alias
/// match, then create. Never merges and never overwrites an existing kind.
pub async fn resolve(pool: &PgPool, label: &str, kind: &str) -> Result<ResolvedEntity, sqlx::Error> {
    let normalized = store::normalize_label(label);
    if let Some(entity) = store::find_entity_by_normalized(pool, &normalized).await? {
        return Ok(ResolvedEntity {
            entity,
            created: false,
            method: ResolutionMethod::Exact,
            ambiguous_candidates: vec![],
        });
    }

    let alias_matches = store::find_entities_by_alias(pool, &normalized).await?;
    match alias_matches.len() {
        1 => Ok(ResolvedEntity {
            entity: alias_matches[0].clone(),
            created: false,
            method: ResolutionMethod::Alias,
            ambiguous_candidates: vec![],
        }),
        _ => {
            let entity = store::insert_entity(pool, label, kind).await?;
            Ok(ResolvedEntity {
                entity,
                created: true,
                method: ResolutionMethod::Created,
                ambiguous_candidates: alias_matches.iter().map(|e| e.id).collect(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_labels() {
        assert_eq!(ResolutionMethod::Exact.as_str(), "exact");
        assert_eq!(ResolutionMethod::Alias.as_str(), "alias");
        assert_eq!(ResolutionMethod::Created.as_str(), "created");
    }
}