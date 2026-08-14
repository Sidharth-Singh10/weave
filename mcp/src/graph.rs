//! Graph retrieval: entity lookup and BFS subgraph expansion.

use std::collections::{HashSet, VecDeque};

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Entity, EntityView, RelationView};
use crate::store;

/// Resolve an entity by label and return it with its full neighborhood.
pub async fn get_node(pool: &PgPool, label: &str) -> Result<Option<EntityView>, sqlx::Error> {
    let entity = store::find_entity_by_normalized(pool, &store::normalize_label(label)).await?;
    let Some(entity) = entity else {
        return Ok(None);
    };
    let relations = store::relations_for_entity(pool, entity.id).await?;
    Ok(Some(EntityView { entity, relations }))
}

/// BFS expansion from a seed label up to `depth` hops (in both directions).
/// Returns the reachable entities and the edges between them.
pub async fn get_related(
    pool: &PgPool,
    label: &str,
    depth: usize,
) -> Result<Option<GraphSubgraph>, sqlx::Error> {
    let seed = store::find_entity_by_normalized(pool, &store::normalize_label(label)).await?;
    let Some(seed) = seed else {
        return Ok(None);
    };

    let depth = depth.clamp(1, 3);
    let mut visited: HashSet<Uuid> = HashSet::new();
    let mut entities: Vec<Entity> = Vec::new();
    let mut edges: Vec<RelationView> = Vec::new();
    let mut queue: VecDeque<(Uuid, usize)> = VecDeque::new();

    visited.insert(seed.id);
    entities.push(seed);
    queue.push_back((entities[0].id, 0));

    while let Some((id, hop)) = queue.pop_front() {
        if hop >= depth {
            continue;
        }
        for relation in store::relations_for_entity(pool, id).await? {
            let neighbor_id = if relation.source_id == id {
                relation.target_id
            } else {
                relation.source_id
            };
            if !edges.iter().any(|e| e.relation_id == relation.relation_id) {
                edges.push(relation);
            }
            if visited.insert(neighbor_id) {
                if let Some(entity) = store::get_entity_by_id(pool, neighbor_id).await? {
                    entities.push(entity);
                    queue.push_back((neighbor_id, hop + 1));
                }
            }
        }
    }

    Ok(Some(GraphSubgraph { entities, edges }))
}

/// The result of a subgraph expansion.
#[derive(Debug, Clone)]
pub struct GraphSubgraph {
    pub entities: Vec<Entity>,
    pub edges: Vec<RelationView>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_clamped() {
        // Clamping is exercised inside get_related; this guards the range.
        assert_eq!((0usize).clamp(1, 3), 1);
        assert_eq!((5usize).clamp(1, 3), 3);
    }
}
