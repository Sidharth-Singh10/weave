use std::collections::{HashSet, VecDeque};

use weave_core::llm::{OpenCodeClient, TokenUsage};
use weave_core::models::{
    GraphEdge, GraphNode, LabelCommunityRequest, LabelCommunityResult, OrganizeRequest,
    OrganizeResult, SearchRequest, SearchResult,
};

const ORGANIZE_PROMPT: &str = r#"You are a knowledge-organization assistant for a learning app.
You are given a knowledge graph (nodes and edges) the user built from natural-language notes.

Suggest improvements. Respond with strict JSON only:
{"groups":[{"label":"category name","member_labels":["node label", ...]}],
 "missing_edges":[{"source_label":"...","target_label":"...","relation":"..."}],
 "disconnected":["node label", ...],
 "duplicates":[{"label_a":"...","label_b":"..."}]}

Rules:
- groups: higher-level categories that group related existing node labels. Use EXACT existing node labels as members.
- missing_edges: plausible relationships not yet in the graph, connecting existing labels only.
- disconnected: existing node labels that have no edges to any other node.
- duplicates: pairs of existing labels that look like the same concept.
- Only suggest high-confidence items. Empty arrays are fine.
"#;

const SEARCH_PROMPT: &str = r#"You are a search assistant for a knowledge graph.
Given the graph below and a user question, pick the node labels from the graph that are most relevant to the question.
Respond with strict JSON only: {"labels":["...", "..."],"rationale":"one short sentence"}
Use EXACT existing node labels. If nothing matches, return an empty labels array.
"#;

const LABEL_COMMUNITY_PROMPT: &str = r#"You are a knowledge-graph organizer.
You are given the node labels of one group of related concepts.
Suggest ONE short group label (1-3 words, title case) that best captures what these concepts have in common.
Respond with strict JSON only: {"label":"..."}
"#;

fn graph_description(nodes: &[GraphNode], edges: &[GraphEdge]) -> String {
    let node_str = nodes
        .iter()
        .map(|n| format!("{} ({})", n.label, if n.kind.is_empty() { "concept" } else { &n.kind }))
        .collect::<Vec<_>>()
        .join(", ");
    let edge_str = edges
        .iter()
        .map(|e| format!("{} -[{}]-> {}", e.source_label, e.relation, e.target_label))
        .collect::<Vec<_>>()
        .join("; ");
    format!("Nodes: [{}]\nEdges: [{}]", node_str, edge_str)
}

pub async fn organize(client: &OpenCodeClient, req: &OrganizeRequest) -> (OrganizeResult, Option<TokenUsage>) {
    if !client.available() {
        return (OrganizeResult::default(), None);
    }
    let user = graph_description(&req.nodes, &req.edges);
    match client.chat_json(ORGANIZE_PROMPT, &user).await {
        Ok(out) => (
            serde_json::from_value(out.json).unwrap_or_default(),
            out.usage,
        ),
        Err(e) => {
            tracing::warn!("organize LLM call failed: {e}");
            (OrganizeResult::default(), None)
        }
    }
}

/// Ask the LLM to name a detected community. Graceful: returns an empty
/// label when the LLM is unavailable or the request fails, so visualization
/// never breaks on a naming failure.
pub async fn label_community(
    client: &OpenCodeClient,
    req: &LabelCommunityRequest,
) -> (LabelCommunityResult, Option<TokenUsage>) {
    if !client.available() || req.nodes.is_empty() {
        return (
            LabelCommunityResult {
                label: String::new(),
            },
            None,
        );
    }
    let user = format!("Node labels: {}", req.nodes.join(", "));
    match client.chat_json(LABEL_COMMUNITY_PROMPT, &user).await {
        Ok(out) => {
            let label = out.json["label"]
                .as_str()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            (LabelCommunityResult { label }, out.usage)
        }
        Err(e) => {
            tracing::warn!("label_community LLM call failed: {e}");
            (
                LabelCommunityResult {
                    label: String::new(),
                },
                None,
            )
        }
    }
}

pub async fn search(client: &OpenCodeClient, req: &SearchRequest) -> (SearchResult, Option<TokenUsage>) {
    if !client.available() {
        return (
            SearchResult {
                matches: vec![],
                rationale: "LLM not configured".to_string(),
            },
            None,
        );
    }

    let user = format!(
        "Graph:\n{}\n\nQuestion: {}",
        graph_description(&req.nodes, &req.edges),
        req.query
    );

    let (seeds, rationale, usage) = match client.chat_json(SEARCH_PROMPT, &user).await {
        Ok(out) => {
            let seeds = out.json["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let rationale = out.json["rationale"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_default();
            (seeds, rationale, out.usage)
        }
        Err(e) => {
            tracing::warn!("search LLM call failed: {e}");
            return (
                SearchResult {
                    matches: vec![],
                    rationale: "search unavailable".to_string(),
                },
                None,
            );
        }
    };

    let matches = expand_neighbors(&seeds, &req.nodes, &req.edges);

    (
        SearchResult {
            matches,
            rationale,
        },
        usage,
    )
}

/// Deterministic expansion: start from the LLM-seeded labels, then include all
/// nodes reachable within two hops. Keeps graph traversal predictable and
/// bounded, independent of the model.
fn expand_neighbors(seeds: &HashSet<String>, nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<String> {
    // Build adjacency on normalized labels.
    let mut adj: std::collections::HashMap<String, Vec<String>> = Default::default();
    for e in edges {
        let a = e.source_label.to_lowercase();
        let b = e.target_label.to_lowercase();
        adj.entry(a.clone()).or_default().push(b.clone());
        adj.entry(b).or_default().push(a);
    }

    let mut visited: HashSet<String> = seeds.iter().map(|s| s.to_lowercase()).collect();
    let mut queue: VecDeque<String> = visited.iter().cloned().collect();
    let mut hops: std::collections::HashMap<String, usize> = visited
        .iter()
        .map(|s| (s.clone(), 0usize))
        .collect();

    while let Some(node) = queue.pop_front() {
        let d = hops[&node];
        if d >= 2 {
            continue;
        }
        if let Some(neighbors) = adj.get(&node) {
            for n in neighbors {
                if visited.insert(n.clone()) {
                    hops.insert(n.clone(), d + 1);
                    queue.push_back(n.clone());
                }
            }
        }
    }

    nodes
        .iter()
        .filter(|n| visited.contains(&n.label.to_lowercase()))
        .map(|n| n.label.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(label: &str) -> GraphNode {
        GraphNode {
            id: None,
            label: label.to_string(),
            kind: String::new(),
        }
    }

    fn edge(src: &str, tgt: &str, rel: &str) -> GraphEdge {
        GraphEdge {
            id: None,
            source_id: None,
            target_id: None,
            source_label: src.to_string(),
            target_label: tgt.to_string(),
            relation: rel.to_string(),
        }
    }

    /// Empty member list must never reach the LLM; returns an empty label.
    #[test]
    fn label_community_empty_nodes_returns_empty() {
        let client = OpenCodeClient::from_env();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(label_community(
                &client,
                &LabelCommunityRequest { nodes: vec![] },
            ));
        assert!(result.0.label.is_empty());
    }

    /// Linear chain: A - B - C - D - E
    /// Seed = {A}
    /// Expected: A (seed, hop 0), B (hop 1), C (hop 2)
    /// Excluded: D (hop 3), E (hop 4)
    #[test]
    fn bfs_stops_at_two_hops() {
        let nodes = vec![node("A"), node("B"), node("C"), node("D"), node("E")];
        let edges = vec![
            edge("A", "B", "r"),
            edge("B", "C", "r"),
            edge("C", "D", "r"),
            edge("D", "E", "r"),
        ];
        let seeds: HashSet<String> = ["A".to_string()].into();
        let mut result = expand_neighbors(&seeds, &nodes, &edges);
        result.sort();
        assert_eq!(result, vec!["A", "B", "C"]);
    }

    /// Same chain but seed = {C} in the middle.
    /// Expected: A (hop 2), B (hop 1), C (seed), D (hop 1), E (hop 2)
    #[test]
    fn bfs_expands_bidirectionally() {
        let nodes = vec![node("A"), node("B"), node("C"), node("D"), node("E")];
        let edges = vec![
            edge("A", "B", "r"),
            edge("B", "C", "r"),
            edge("C", "D", "r"),
            edge("D", "E", "r"),
        ];
        let seeds: HashSet<String> = ["C".to_string()].into();
        let mut result = expand_neighbors(&seeds, &nodes, &edges);
        result.sort();
        assert_eq!(result, vec!["A", "B", "C", "D", "E"]);
    }

    /// Disconnected node Z has no edges — never reached.
    #[test]
    fn disconnected_node_excluded() {
        let nodes = vec![node("A"), node("B"), node("Z")];
        let edges = vec![edge("A", "B", "r")];
        let seeds: HashSet<String> = ["A".to_string()].into();
        let result = expand_neighbors(&seeds, &nodes, &edges);
        assert!(result.contains(&"A".to_string()));
        assert!(result.contains(&"B".to_string()));
        assert!(!result.contains(&"Z".to_string()));
    }

    /// Case-insensitive: seed "harry potter" matches node "Harry Potter".
    #[test]
    fn case_insensitive_seed_match() {
        let nodes = vec![node("Harry Potter"), node("Ron"), node("Hermione")];
        let edges = vec![
            edge("Harry Potter", "Ron", "friend of"),
            edge("Harry Potter", "Hermione", "friend of"),
        ];
        let seeds: HashSet<String> = ["harry potter".to_string()].into();
        let mut result = expand_neighbors(&seeds, &nodes, &edges);
        result.sort();
        assert_eq!(result, vec!["Harry Potter", "Hermione", "Ron"]);
    }

    /// Multiple seeds cover more of the graph.
    #[test]
    fn multiple_seeds() {
        // Star: X connects to A,B,C,D each of which connects to a leaf.
        // A-A1, B-B1, C-C1, D-D1, all through X.
        let nodes = vec![
            node("X"),
            node("A"), node("A1"),
            node("B"), node("B1"),
            node("C"), node("C1"),
            node("D"), node("D1"),
        ];
        let edges = vec![
            edge("X", "A", "r"), edge("A", "A1", "r"),
            edge("X", "B", "r"), edge("B", "B1", "r"),
            edge("X", "C", "r"), edge("C", "C1", "r"),
            edge("X", "D", "r"), edge("D", "D1", "r"),
        ];
        // Seed = {A, D} → hop 0: A,D; hop 1: A1, X, D1; hop 2: B, C
        // B1 and C1 are hop 3 from either seed → excluded
        let seeds: HashSet<String> = ["A".to_string(), "D".to_string()].into();
        let mut result = expand_neighbors(&seeds, &nodes, &edges);
        result.sort();
        assert_eq!(result, vec!["A", "A1", "B", "C", "D", "D1", "X"]);
        assert!(!result.contains(&"B1".to_string()));
        assert!(!result.contains(&"C1".to_string()));
    }

    /// Empty seeds → empty result.
    #[test]
    fn empty_seeds_returns_nothing() {
        let nodes = vec![node("A"), node("B")];
        let edges = vec![edge("A", "B", "r")];
        let seeds: HashSet<String> = HashSet::new();
        let result = expand_neighbors(&seeds, &nodes, &edges);
        assert!(result.is_empty());
    }

    /// No edges → only the seed itself returned.
    #[test]
    fn no_edges_returns_only_seed() {
        let nodes = vec![node("A"), node("B"), node("C")];
        let edges: Vec<GraphEdge> = vec![];
        let seeds: HashSet<String> = ["A".to_string()].into();
        let result = expand_neighbors(&seeds, &nodes, &edges);
        assert_eq!(result, vec!["A"]);
    }
}
