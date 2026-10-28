use std::collections::{HashMap, HashSet, VecDeque};

use weave_core::analyze::{self, Analysis};
use weave_core::llm::{OpenCodeClient, TokenUsage};
use weave_core::models::{
    DuplicatePair, GraphEdge, GraphNode, LabelCommunityRequest, LabelCommunityResult,
    OrganizeRequest, OrganizeResult, SearchRequest, SearchResult,
};

const ORGANIZE_PROMPT: &str = r#"You are a knowledge-organization assistant for a learning app.
You are given a compact digest of a knowledge graph the user built from natural-language notes:
its most connected concepts, its bridge concepts, isolated concepts, suspected duplicate labels,
and a sample of existing relationships.

Suggest higher-level organization. Respond with strict JSON only:
{"groups":[{"label":"category name","member_labels":["node label", ...]}],
 "missing_edges":[{"source_label":"...","target_label":"...","relation":"..."}]}

Rules:
- groups: higher-level categories that group related existing concepts. Use EXACT labels from the digest.
- missing_edges: plausible relationships between labels that already appear in the digest. Never invent a label.
- Only suggest high-confidence items. Empty arrays are fine.
- Isolated concepts and duplicate labels are already reported separately; do not restate them.
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

/// Bounded graph digest for the search prompt.
///
/// The whole graph used to be serialized here, so prompt cost grew with the
/// canvas. Search is already seeded by semantic hints, so the model only needs
/// the labels around those hints — plus the most connected concepts as a
/// fallback when there are none.
fn search_digest(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    hints: &[serde_json::Value],
) -> String {
    const MAX_LABELS: usize = 120;
    const MAX_EDGES: usize = 150;

    let kind_of: HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.label.as_str(), n.kind.as_str()))
        .collect();
    let canonical: HashMap<String, &str> = nodes
        .iter()
        .map(|n| (n.label.to_lowercase(), n.label.as_str()))
        .collect();

    // Hints arrive from the embedder and may not match an existing label.
    let hint_labels: Vec<String> = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .filter_map(|label| canonical.get(&label.to_lowercase()).map(|s| s.to_string()))
        .collect();

    let labels: Vec<String> = if hint_labels.is_empty() {
        let degree = analyze::degree_map(nodes, edges);
        let mut ranked: Vec<(String, usize)> = nodes
            .iter()
            .map(|n| (n.label.clone(), degree.get(&n.label).copied().unwrap_or(0)))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked
            .into_iter()
            .take(MAX_LABELS)
            .map(|(label, _)| label)
            .collect()
    } else {
        let all: Vec<String> = nodes.iter().map(|n| n.label.clone()).collect();
        analyze::neighbourhood(&all, edges, &hint_labels, 1)
            .into_iter()
            .take(MAX_LABELS)
            .collect()
    };

    let shown: HashSet<&str> = labels.iter().map(|s| s.as_str()).collect();
    let visible_edges: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| {
            shown.contains(e.source_label.as_str()) && shown.contains(e.target_label.as_str())
        })
        .collect();

    let node_str = labels
        .iter()
        .map(|label| {
            let kind = kind_of.get(label.as_str()).copied().unwrap_or("");
            if kind.is_empty() {
                format!("{label} (concept)")
            } else {
                format!("{label} ({kind})")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let edge_str = visible_edges
        .iter()
        .take(MAX_EDGES)
        .map(|e| format!("{} -[{}]-> {}", e.source_label, e.relation, e.target_label))
        .collect::<Vec<_>>()
        .join("; ");

    format!(
        "Nodes ({} of {}): [{}]\nEdges ({} of {}): [{}]",
        labels.len(),
        nodes.len(),
        node_str,
        visible_edges.len().min(MAX_EDGES),
        edges.len(),
        edge_str
    )
}

pub async fn organize(client: &OpenCodeClient, req: &OrganizeRequest) -> (OrganizeResult, Option<TokenUsage>) {
    // Deterministic pass first: what the graph's own structure already tells us.
    // These findings are never delegated to the model.
    let analysis = analyze::analyze(&req.nodes, &req.edges);
    let mut result = OrganizeResult {
        groups: vec![],
        missing_edges: vec![],
        disconnected: analysis.disconnected.clone(),
        duplicates: analysis
            .near_duplicates
            .iter()
            .map(|(a, b)| DuplicatePair {
                label_a: a.clone(),
                label_b: b.clone(),
            })
            .collect(),
    };

    if !client.available() {
        return (result, None);
    }

    // Only a bounded digest reaches the model, so prompt cost stops scaling
    // with graph size.
    let user = organize_digest(&req.edges, &analysis);
    match client.chat_json(ORGANIZE_PROMPT, &user).await {
        Ok(out) => {
            let narrated: OrganizeResult = serde_json::from_value(out.json).unwrap_or_default();
            let known: HashSet<&str> = req.nodes.iter().map(|n| n.label.as_str()).collect();

            // A suggestion may only reference concepts that exist.
            result.groups = narrated
                .groups
                .into_iter()
                .filter(|g| g.member_labels.iter().any(|m| known.contains(m.as_str())))
                .collect();
            result.missing_edges = narrated
                .missing_edges
                .into_iter()
                .filter(|e| {
                    known.contains(e.source_label.as_str())
                        && known.contains(e.target_label.as_str())
                })
                .collect();
            (result, out.usage)
        }
        Err(e) => {
            tracing::warn!("organize LLM call failed: {e}");
            (result, None)
        }
    }
}

/// The bounded digest handed to the model: deterministic signals plus a sample
/// of relationships anchored on the most connected concepts.
fn organize_digest(edges: &[GraphEdge], analysis: &Analysis) -> String {
    const MAX_EDGE_SAMPLE: usize = 60;

    let join_ranked = |items: &[analyze::Ranked]| -> String {
        if items.is_empty() {
            "(none)".to_string()
        } else {
            items
                .iter()
                .map(|r| format!("{} ({})", r.label, r.degree))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };

    let sample_list = |items: &[String]| -> String {
        if items.is_empty() {
            return "(none)".to_string();
        }
        let mut s = items.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
        if items.len() > 8 {
            s.push_str(&format!(", … {} more", items.len() - 8));
        }
        s
    };

    let duplicates = if analysis.near_duplicates.is_empty() {
        "(none)".to_string()
    } else {
        analysis
            .near_duplicates
            .iter()
            .take(8)
            .map(|(a, b)| format!("{a} ≈ {b}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // Prefer edges touching a god node: those carry the most structure.
    let god: Vec<&str> = analysis.god_nodes.iter().map(|r| r.label.as_str()).collect();
    let mut sample: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| god.contains(&e.source_label.as_str()) || god.contains(&e.target_label.as_str()))
        .take(MAX_EDGE_SAMPLE)
        .collect();
    if sample.len() < MAX_EDGE_SAMPLE {
        for e in edges {
            if sample.len() >= MAX_EDGE_SAMPLE {
                break;
            }
            if !sample.iter().any(|s| std::ptr::eq(*s, e)) {
                sample.push(e);
            }
        }
    }
    let edge_str = if sample.is_empty() {
        "(none)".to_string()
    } else {
        sample
            .iter()
            .map(|e| format!("{} -[{}]-> {}", e.source_label, e.relation, e.target_label))
            .collect::<Vec<_>>()
            .join("; ")
    };

    format!(
        "MOST CONNECTED\n{god}\n\nBRIDGES (removal splits the graph)\n{bridges}\n\nISOLATED\n{isolated}\n\nDUPLICATE CANDIDATES\n{duplicates}\n\nRELATIONSHIPS ({edges} of {total})\n{edge_str}",
        god = join_ranked(&analysis.god_nodes),
        bridges = join_ranked(&analysis.bridge_nodes),
        isolated = sample_list(&analysis.disconnected),
        duplicates = duplicates,
        edges = sample.len(),
        total = edges.len(),
        edge_str = edge_str,
    )
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

pub async fn search(
    client: &OpenCodeClient,
    req: &SearchRequest,
    semantic_hints: &[serde_json::Value],
) -> (SearchResult, Option<TokenUsage>) {
    if !client.available() {
        return (
            SearchResult {
                matches: vec![],
                rationale: "LLM not configured".to_string(),
            },
            None,
        );
    }

    let hint_str = if semantic_hints.is_empty() {
        "(none)".to_string()
    } else {
        semantic_hints
            .iter()
            .filter_map(|h| h["label"].as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let user = format!(
        "Graph:\n{}\n\nQuestion: {}\n\nSemantic hints (likely relevant labels): {}",
        search_digest(&req.nodes, &req.edges, semantic_hints),
        req.query,
        hint_str
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
            ..GraphEdge::default()
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

    // -- deterministic organize ---------------------------------------------

    /// Without an LLM the structural findings must still be reported: they are
    /// computed from the graph, never delegated to the model.
    #[test]
    fn organize_reports_deterministic_findings_without_llm() {
        let client = OpenCodeClient::mock();
        let req = OrganizeRequest {
            nodes: vec![node("PostgreSQL"), node("Postgres"), node("Orphan")],
            edges: vec![],
        };
        let (result, usage) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(organize(&client, &req));

        assert!(usage.is_none(), "unavailable LLM must not report usage");
        assert!(
            result.disconnected.contains(&"Orphan".to_string()),
            "isolated node must be reported"
        );
        assert!(
            result
                .duplicates
                .iter()
                .any(|d| d.label_a == "PostgreSQL" && d.label_b == "Postgres"),
            "near-duplicate labels must be reported"
        );
    }

    /// The narration prompt carries a bounded sample, not the whole graph.
    #[test]
    fn organize_digest_is_bounded() {
        let nodes: Vec<GraphNode> = (0..200)
            .map(|i| node(&format!("Concept {i}")))
            .collect();
        let edges: Vec<GraphEdge> = (0..200)
            .map(|i| edge(&format!("Concept {i}"), &format!("Concept {}", (i + 1) % 200), "relates to"))
            .collect();

        let analysis = analyze::analyze(&nodes, &edges);
        let digest = organize_digest(&edges, &analysis);

        assert!(
            digest.contains("RELATIONSHIPS (60 of 200)"),
            "digest must cap the relationship sample: {digest}"
        );
    }

    // -- bounded search prompt -----------------------------------------------

    fn concept_graph(n: usize) -> (Vec<GraphNode>, Vec<GraphEdge>) {
        let nodes: Vec<GraphNode> = (0..n).map(|i| node(&format!("Concept {i}"))).collect();
        let edges: Vec<GraphEdge> = (0..n)
            .map(|i| {
                edge(
                    &format!("Concept {i}"),
                    &format!("Concept {}", (i + 1) % n),
                    "relates to",
                )
            })
            .collect();
        (nodes, edges)
    }

    /// Hints scope the digest to their neighborhood instead of the whole graph.
    #[test]
    fn search_digest_follows_hints() {
        let (nodes, edges) = concept_graph(300);
        let hints = vec![serde_json::json!({ "label": "Concept 42" })];

        let digest = search_digest(&nodes, &edges, &hints);

        assert!(
            digest.contains("Nodes (3 of 300)"),
            "only the hint plus its two neighbors should be shown: {digest}"
        );
        assert!(
            !digest.contains("Concept 200"),
            "unrelated labels must not be sent to the model"
        );
    }

    /// Without hints, the digest falls back to the most connected labels and
    /// still respects the cap.
    #[test]
    fn search_digest_without_hints_is_capped() {
        let (nodes, edges) = concept_graph(300);

        let digest = search_digest(&nodes, &edges, &[]);

        assert!(
            digest.starts_with("Nodes (120 of 300)"),
            "fallback must cap labels: {digest}"
        );
    }
}
