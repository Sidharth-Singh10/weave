//! Graph retrieval: entity lookup and BFS subgraph expansion.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::Serialize;
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

// ---------------------------------------------------------------------------
// Label lookup: never guess between candidates
// ---------------------------------------------------------------------------

/// An exact normalized-label hit, or the near matches a caller can choose from.
/// Read tools surface `candidates` instead of inventing an answer from a fuzzy
/// match — the same discipline as refusing an ambiguous name outright.
#[derive(Debug, Clone)]
pub enum LabelLookup {
    Found(Entity),
    NotFound { candidates: Vec<Entity> },
}

pub async fn lookup(pool: &PgPool, label: &str) -> Result<LabelLookup, sqlx::Error> {
    let normalized = store::normalize_label(label);
    if let Some(entity) = store::find_entity_by_normalized(pool, &normalized).await? {
        return Ok(LabelLookup::Found(entity));
    }
    let candidates = store::similar_entities(pool, &normalized, None, 5).await?;
    Ok(LabelLookup::NotFound { candidates })
}

// ---------------------------------------------------------------------------
// Explain: one node, its degree, and every relation touching it
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Connection {
    pub relation: String,
    /// Label of the endpoint on the other side.
    pub other: String,
    /// `outgoing` when this entity is the subject, `incoming` when the object.
    pub direction: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Explanation {
    pub entity: Entity,
    pub degree: usize,
    pub outgoing: Vec<Connection>,
    pub incoming: Vec<Connection>,
}

/// Outcome of an explain query.
#[derive(Debug, Clone)]
pub enum ExplainOutcome {
    Found(Explanation),
    NotFound { candidates: Vec<Entity> },
}

/// Everything the graph knows about one entity: its neighbours, in both
/// directions, with the relation that links them. Deterministic and cheap —
/// no LLM call, no retrieval scoring.
pub async fn explain(pool: &PgPool, label: &str) -> Result<ExplainOutcome, sqlx::Error> {
    let entity = match lookup(pool, label).await? {
        LabelLookup::Found(entity) => entity,
        LabelLookup::NotFound { candidates } => {
            return Ok(ExplainOutcome::NotFound { candidates });
        }
    };

    let mut outgoing = Vec::new();
    let mut incoming = Vec::new();
    for r in store::relations_for_entity(pool, entity.id).await? {
        if r.source_id == entity.id {
            outgoing.push(Connection {
                relation: r.relation,
                other: r.target_label,
                direction: "outgoing",
            });
        } else {
            incoming.push(Connection {
                relation: r.relation,
                other: r.source_label,
                direction: "incoming",
            });
        }
    }

    let degree = outgoing.len() + incoming.len();
    Ok(ExplainOutcome::Found(Explanation {
        entity,
        degree,
        outgoing,
        incoming,
    }))
}

// ---------------------------------------------------------------------------
// Path: how are two things connected?
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct PathHop {
    pub from: String,
    pub relation: String,
    pub to: String,
    /// `forward` when the stored relation runs from → to, `reverse` when the
    /// path traverses it backwards.
    pub direction: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathResult {
    pub from: String,
    pub to: String,
    pub length: usize,
    pub hops: Vec<PathHop>,
    /// Rendered chain, e.g. `A --studies at--> B <--friend of-- C`.
    pub chain: String,
}

/// Outcome of a path query. A missing label carries its candidates so the
/// caller can retry with a real one instead of guessing.
#[derive(Debug, Clone)]
pub enum PathOutcome {
    Found(PathResult),
    NoPath,
    Missing { label: String, candidates: Vec<Entity> },
}

const MAX_PATH_DEPTH: usize = 8;
const MAX_PATH_VISITS: usize = 5_000;

/// Shortest path between two entities over undirected relations.
pub async fn find_path(
    pool: &PgPool,
    from_label: &str,
    to_label: &str,
    max_depth: usize,
) -> Result<PathOutcome, sqlx::Error> {
    let from = match lookup(pool, from_label).await? {
        LabelLookup::Found(e) => e,
        LabelLookup::NotFound { candidates } => {
            return Ok(PathOutcome::Missing {
                label: from_label.to_string(),
                candidates,
            });
        }
    };
    let to = match lookup(pool, to_label).await? {
        LabelLookup::Found(e) => e,
        LabelLookup::NotFound { candidates } => {
            return Ok(PathOutcome::Missing {
                label: to_label.to_string(),
                candidates,
            });
        }
    };

    if from.id == to.id {
        return Ok(PathOutcome::Found(PathResult {
            from: from.label.clone(),
            to: to.label.clone(),
            length: 0,
            hops: vec![],
            chain: from.label,
        }));
    }

    let max_depth = max_depth.clamp(1, MAX_PATH_DEPTH);

    // BFS outward from `from`, remembering how each node was first reached.
    let mut parent: HashMap<Uuid, (Uuid, RelationView)> = HashMap::new();
    let mut seen: HashSet<Uuid> = [from.id].into();
    let mut queue: VecDeque<(Uuid, usize)> = VecDeque::from([(from.id, 0usize)]);
    let mut found = false;

    while let Some((id, depth)) = queue.pop_front() {
        if id == to.id {
            found = true;
            break;
        }
        if depth >= max_depth || seen.len() >= MAX_PATH_VISITS {
            continue;
        }
        for relation in store::relations_for_entity(pool, id).await? {
            let neighbor = if relation.source_id == id {
                relation.target_id
            } else {
                relation.source_id
            };
            if seen.insert(neighbor) {
                parent.insert(neighbor, (id, relation));
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    if !found {
        return Ok(PathOutcome::NoPath);
    }

    // Walk back from the target, then flip into traversal order.
    let mut reversed_hops: Vec<PathHop> = Vec::new();
    let mut cursor = to.id;
    while cursor != from.id {
        let Some((prev, relation)) = parent.get(&cursor) else {
            return Ok(PathOutcome::NoPath);
        };
        let forward = relation.source_id == *prev;
        reversed_hops.push(PathHop {
            from: if forward {
                relation.source_label.clone()
            } else {
                relation.target_label.clone()
            },
            relation: relation.relation.clone(),
            to: if forward {
                relation.target_label.clone()
            } else {
                relation.source_label.clone()
            },
            direction: if forward { "forward" } else { "reverse" },
        });
        cursor = *prev;
    }
    reversed_hops.reverse();

    let mut chain = from.label.clone();
    for hop in &reversed_hops {
        if hop.direction == "forward" {
            chain.push_str(&format!(" --{}--> {}", hop.relation, hop.to));
        } else {
            chain.push_str(&format!(" <--{}-- {}", hop.relation, hop.to));
        }
    }

    Ok(PathOutcome::Found(PathResult {
        from: from.label,
        to: to.label,
        length: reversed_hops.len(),
        hops: reversed_hops,
        chain,
    }))
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

    #[test]
    fn path_depth_is_bounded() {
        assert_eq!(MAX_PATH_DEPTH, 8);
        assert_eq!((0usize).clamp(1, MAX_PATH_DEPTH), 1);
        assert_eq!((99usize).clamp(1, MAX_PATH_DEPTH), MAX_PATH_DEPTH);
    }
}
