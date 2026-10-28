//! Deterministic graph analysis.
//!
//! Every signal here is computed from graph structure alone — no LLM, no
//! vectors, nothing that varies between runs. The backend uses these results
//! as *candidates* and asks a model only to narrate them, which keeps the
//! prompt bounded (cost no longer scales with graph size) and makes it
//! impossible for a suggestion to reference a concept that doesn't exist.
//!
//! Signals:
//! - **God nodes** — the most connected concepts, hubs excluded by percentile.
//! - **Bridge nodes** — articulation points: remove one and the graph splits.
//! - **Disconnected** — nodes with no relationships at all.
//! - **Near-duplicates** — labels that differ only by form ("Postgres" /
//!   "PostgreSQL"), the cheapest source of graph rot.
//! - **Uncertain edges** — `INFERRED` / `AMBIGUOUS` relationships for review.
//! - **Suggested questions** — what the graph is uniquely positioned to answer.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::models::{confidence, GraphEdge, GraphNode};

/// Node kinds that carry no structural signal on their own.
const GENERIC_KINDS: &[&str] = &["", "concept"];

/// Above this many nodes we skip the quadratic duplicate scan rather than
/// stall the request; the other signals are all linear or near-linear.
const DUPLICATE_SCAN_LIMIT: usize = 2_000;

/// One ranked concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub label: String,
    pub degree: usize,
}

/// Everything the deterministic pass found, ready to be narrated.
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub god_nodes: Vec<Ranked>,
    pub bridge_nodes: Vec<Ranked>,
    pub disconnected: Vec<String>,
    /// Pairs of existing labels that look like the same concept.
    pub near_duplicates: Vec<(String, String)>,
    /// Edges whose relation is not grounded in the source note.
    pub uncertain_edges: Vec<UncertainEdge>,
    pub suggested_questions: Vec<String>,
    /// True when the duplicate scan was skipped for size.
    pub duplicate_scan_skipped: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UncertainEdge {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub confidence: String,
}

/// Build the full deterministic picture for a graph.
pub fn analyze(nodes: &[GraphNode], edges: &[GraphEdge]) -> Analysis {
    let degree = degree_map(nodes, edges);
    let near_duplicates = if nodes.len() > DUPLICATE_SCAN_LIMIT {
        Vec::new()
    } else {
        near_duplicates(nodes)
    };

    let disconnected: Vec<String> = nodes
        .iter()
        .filter(|n| degree.get(&n.label).copied().unwrap_or(0) == 0)
        .map(|n| n.label.clone())
        .collect();

    let uncertain_edges: Vec<UncertainEdge> = edges
        .iter()
        .filter(|e| e.confidence != confidence::EXTRACTED)
        .map(|e| UncertainEdge {
            source: e.source_label.clone(),
            relation: e.relation.clone(),
            target: e.target_label.clone(),
            confidence: e.confidence.clone(),
        })
        .collect();

    let god_nodes = god_nodes(nodes, &degree, 10, None);
    let bridge_nodes = bridge_nodes(nodes, edges, &degree, 5);

    let mut analysis = Analysis {
        god_nodes,
        bridge_nodes,
        disconnected,
        near_duplicates,
        uncertain_edges,
        suggested_questions: Vec::new(),
        duplicate_scan_skipped: nodes.len() > DUPLICATE_SCAN_LIMIT,
    };
    analysis.suggested_questions = suggest_questions(&analysis);
    analysis
}

/// Incident-edge count per label (undirected).
pub fn degree_map(nodes: &[GraphNode], edges: &[GraphEdge]) -> HashMap<String, usize> {
    let mut degree: HashMap<String, usize> = nodes
        .iter()
        .map(|n| (n.label.clone(), 0usize))
        .collect();
    for e in edges {
        *degree.entry(e.source_label.clone()).or_insert(0) += 1;
        *degree.entry(e.target_label.clone()).or_insert(0) += 1;
    }
    degree
}

/// The most connected real concepts.
///
/// `exclude_hubs_percentile` suppresses nodes whose degree sits above the given
/// percentile of the degree distribution, so one utility concept that links
/// half the graph doesn't displace the actual subject matter.
pub fn god_nodes(
    nodes: &[GraphNode],
    degree: &HashMap<String, usize>,
    top_n: usize,
    exclude_hubs_percentile: Option<u8>,
) -> Vec<Ranked> {
    let kind_of: HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.label.as_str(), n.kind.as_str()))
        .collect();

    let hub_threshold = exclude_hubs_percentile.and_then(|pct| {
        let mut degrees: Vec<usize> = nodes
            .iter()
            .map(|n| degree.get(&n.label).copied().unwrap_or(0))
            .collect();
        degrees.sort_unstable();
        if degrees.is_empty() {
            return None;
        }
        let idx = (degrees.len() * pct as usize / 100).saturating_sub(1);
        degrees.get(idx).copied()
    });

    let mut ranked: Vec<Ranked> = nodes
        .iter()
        .filter(|n| !GENERIC_KINDS.contains(&n.kind.as_str()) || n.kind.is_empty())
        .map(|n| Ranked {
            label: n.label.clone(),
            degree: degree.get(&n.label).copied().unwrap_or(0),
        })
        .filter(|r| r.degree > 0)
        .filter(|r| hub_threshold.map(|t| r.degree <= t).unwrap_or(true))
        .collect();

    // A generic "concept" node still counts when it is all we have, so keep
    // them only if the typed ranking came up short.
    if ranked.len() < top_n {
        let seen: HashSet<String> = ranked.iter().map(|r| r.label.clone()).collect();
        let mut generics: Vec<Ranked> = nodes
            .iter()
            .filter(|n| !seen.contains(&n.label))
            .filter(|n| {
                let k = kind_of.get(n.label.as_str()).copied().unwrap_or("");
                GENERIC_KINDS.contains(&k)
            })
            .map(|n| Ranked {
                label: n.label.clone(),
                degree: degree.get(&n.label).copied().unwrap_or(0),
            })
            .filter(|r| r.degree > 0)
            .filter(|r| hub_threshold.map(|t| r.degree <= t).unwrap_or(true))
            .collect();
        generics.sort_by(|a, b| b.degree.cmp(&a.degree).then(a.label.cmp(&b.label)));
        ranked.extend(generics);
    }

    ranked.sort_by(|a, b| b.degree.cmp(&a.degree).then(a.label.cmp(&b.label)));
    ranked.truncate(top_n);
    ranked
}

/// Articulation points, ranked by degree. Removing one of these splits its
/// component — the structural meaning of "everything flows through here".
pub fn bridge_nodes(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    degree: &HashMap<String, usize>,
    top_n: usize,
) -> Vec<Ranked> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for n in nodes {
        adj.entry(n.label.as_str()).or_default();
    }
    for e in edges {
        if e.source_label == e.target_label {
            continue;
        }
        adj.entry(e.source_label.as_str())
            .or_default()
            .push(e.target_label.as_str());
        adj.entry(e.target_label.as_str())
            .or_default()
            .push(e.source_label.as_str());
    }

    let mut discovery: HashMap<&str, usize> = HashMap::new();
    let mut low: HashMap<&str, usize> = HashMap::new();
    let mut points: HashSet<&str> = HashSet::new();
    let mut time = 0usize;

    // Iterative Tarjan, to stay safe on long chains.
    let labels: Vec<&str> = adj.keys().copied().collect();
    for root in labels {
        if discovery.contains_key(root) {
            continue;
        }
        discovery.insert(root, time);
        low.insert(root, time);
        time += 1;
        let mut root_children = 0usize;
        let mut stack: Vec<(&str, usize)> = vec![(root, 0)];

        while let Some((node, index)) = stack.pop() {
            let neighbours = adj.get(node).cloned().unwrap_or_default();
            if index < neighbours.len() {
                let next = neighbours[index];
                stack.push((node, index + 1));
                if !discovery.contains_key(next) {
                    if node == root {
                        root_children += 1;
                    }
                    discovery.insert(next, time);
                    low.insert(next, time);
                    time += 1;
                    stack.push((next, 0));
                } else {
                    // Back edge: lift the low-link value.
                    let back = discovery[next];
                    if let Some(l) = low.get_mut(node) {
                        *l = (*l).min(back);
                    }
                }
            } else {
                // Finished this node: propagate low-link to the parent.
                if let Some(&(parent, _)) = stack.last() {
                    let node_low = low[node];
                    if let Some(p) = low.get_mut(parent) {
                        *p = (*p).min(node_low);
                    }
                    if parent != root && node_low >= discovery[parent] {
                        points.insert(parent);
                    }
                }
                if node == root && root_children > 1 {
                    points.insert(root);
                }
            }
        }
    }

    let mut ranked: Vec<Ranked> = points
        .into_iter()
        .map(|label| Ranked {
            label: label.to_string(),
            degree: degree.get(label).copied().unwrap_or(0),
        })
        .collect();
    ranked.sort_by(|a, b| b.degree.cmp(&a.degree).then(a.label.cmp(&b.label)));
    ranked.truncate(top_n);
    ranked
}

/// Labels that look like the same concept: normalised containment or a small
/// edit distance. Deterministic, so the same graph always reports the same
/// pairs.
pub fn near_duplicates(nodes: &[GraphNode]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let labels: Vec<(&str, String)> = nodes
        .iter()
        .map(|n| (n.label.as_str(), normalise(&n.label)))
        .filter(|(_, norm)| !norm.is_empty())
        .collect();

    for i in 0..labels.len() {
        for j in (i + 1)..labels.len() {
            let (a_label, a) = &labels[i];
            let (b_label, b) = &labels[j];

            let contained = a.len() >= 4
                && b.len() >= 4
                && (a.contains(b.as_str()) || b.contains(a.as_str()));
            let close = {
                let (a_len, b_len) = (a.chars().count(), b.chars().count());
                let gap = a_len.abs_diff(b_len);
                gap <= 2 && levenshtein_within(a, b, 2)
            };

            if contained || close {
                // Report the longer label first for a stable reading order.
                let (first, second) = if a_label.len() >= b_label.len() {
                    (*a_label, *b_label)
                } else {
                    (*b_label, *a_label)
                };
                out.push((first.to_string(), second.to_string()));
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

fn normalise(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Levenshtein distance bounded by `max` (returns false once exceeded).
fn levenshtein_within(a: &str, b: &str, max: usize) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return false;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(cur[j]);
        }
        if row_min > max {
            return false;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()] <= max
}

/// Questions this graph is positioned to answer, one per distinct signal.
fn suggest_questions(analysis: &Analysis) -> Vec<String> {
    let mut questions = Vec::new();

    if let Some(bridge) = analysis.bridge_nodes.first() {
        questions.push(format!(
            "Everything routes through \"{}\" — is it really one concept, or several?",
            bridge.label
        ));
    }

    if let Some(dup) = analysis.near_duplicates.first() {
        questions.push(format!(
            "Are \"{}\" and \"{}\" the same thing?",
            dup.0, dup.1
        ));
    }

    if let Some(edge) = analysis
        .uncertain_edges
        .iter()
        .find(|e| e.confidence == confidence::AMBIGUOUS)
    {
        questions.push(format!(
            "What exactly connects \"{}\" and \"{}\"? The relation \"{}\" was never stated in a note.",
            edge.source, edge.target, edge.relation
        ));
    }

    if analysis.disconnected.len() >= 3 {
        let sample: Vec<&str> = analysis
            .disconnected
            .iter()
            .take(3)
            .map(|s| s.as_str())
            .collect();
        questions.push(format!(
            "What do {} have in common with the rest of the graph?",
            sample
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else if let Some(god) = analysis.god_nodes.first() {
        if god.degree >= 5 {
            questions.push(format!(
                "\"{}\" connects to {} concepts — which of those relationships matter most?",
                god.label, god.degree
            ));
        }
    }

    questions
}

/// Neighbours of a label within `depth` hops, used to bound narration prompts.
pub fn neighbourhood(labels: &[String], edges: &[GraphEdge], seeds: &[String], depth: usize) -> Vec<String> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        adj.entry(e.source_label.as_str())
            .or_default()
            .push(e.target_label.as_str());
        adj.entry(e.target_label.as_str())
            .or_default()
            .push(e.source_label.as_str());
    }

    let mut seen: HashSet<&str> = seeds.iter().map(|s| s.as_str()).collect();
    let mut queue: VecDeque<(&str, usize)> = seeds
        .iter()
        .map(|s| (s.as_str(), 0usize))
        .collect();

    while let Some((node, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        for next in adj.get(node).cloned().unwrap_or_default() {
            if seen.insert(next) {
                queue.push_back((next, d + 1));
            }
        }
    }

    labels
        .iter()
        .filter(|l| seen.contains(l.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(label: &str, kind: &str) -> GraphNode {
        GraphNode {
            id: None,
            label: label.to_string(),
            kind: kind.to_string(),
        }
    }

    fn edge(a: &str, b: &str, rel: &str) -> GraphEdge {
        GraphEdge {
            source_label: a.to_string(),
            target_label: b.to_string(),
            relation: rel.to_string(),
            ..GraphEdge::default()
        }
    }

    #[test]
    fn god_nodes_rank_by_degree() {
        let nodes = vec![node("Hub", "person"), node("A", "person"), node("B", "person")];
        let edges = vec![edge("Hub", "A", "knows"), edge("Hub", "B", "knows")];
        let degree = degree_map(&nodes, &edges);
        let ranked = god_nodes(&nodes, &degree, 5, None);
        assert_eq!(ranked[0].label, "Hub");
        assert_eq!(ranked[0].degree, 2);
    }

    #[test]
    fn hub_percentile_suppresses_utility_nodes() {
        let nodes = vec![node("Hub", "person"), node("A", "person"), node("B", "person")];
        let edges = vec![edge("Hub", "A", "knows"), edge("Hub", "B", "knows")];
        let degree = degree_map(&nodes, &edges);
        // Hub sits above the 50th percentile and is excluded.
        let ranked = god_nodes(&nodes, &degree, 5, Some(50));
        assert!(ranked.iter().all(|r| r.label != "Hub"));
    }

    #[test]
    fn articulation_point_is_a_bridge() {
        // A - B - C: B is the only node whose removal disconnects the chain.
        let nodes = vec![node("A", "concept"), node("B", "concept"), node("C", "concept")];
        let edges = vec![edge("A", "B", "r"), edge("B", "C", "r")];
        let degree = degree_map(&nodes, &edges);
        let bridges = bridge_nodes(&nodes, &edges, &degree, 5);
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].label, "B");
    }

    #[test]
    fn cycle_has_no_bridge() {
        let nodes = vec![node("A", "concept"), node("B", "concept"), node("C", "concept")];
        let edges = vec![
            edge("A", "B", "r"),
            edge("B", "C", "r"),
            edge("C", "A", "r"),
        ];
        let degree = degree_map(&nodes, &edges);
        assert!(bridge_nodes(&nodes, &edges, &degree, 5).is_empty());
    }

    #[test]
    fn square_cycle_has_no_bridge() {
        // A-B-C-D-A: every node has an alternative route, so none is a bridge.
        let nodes = vec![
            node("A", "concept"),
            node("B", "concept"),
            node("C", "concept"),
            node("D", "concept"),
        ];
        let edges = vec![
            edge("A", "B", "r"),
            edge("B", "C", "r"),
            edge("C", "D", "r"),
            edge("D", "A", "r"),
        ];
        let degree = degree_map(&nodes, &edges);
        assert!(bridge_nodes(&nodes, &edges, &degree, 5).is_empty());
    }

    #[test]
    fn near_duplicates_find_containment_and_typos() {
        let nodes = vec![
            node("PostgreSQL", "concept"),
            node("Postgres", "concept"),
            node("Harry Potter", "person"),
            node("Hermione", "person"),
        ];
        let dups = near_duplicates(&nodes);
        assert!(dups.contains(&("PostgreSQL".to_string(), "Postgres".to_string())));
        assert!(!dups.iter().any(|(a, b)| a == "Harry Potter" && b == "Hermione"));
    }

    #[test]
    fn analysis_collects_uncertain_edges_and_disconnected() {
        let nodes = vec![node("A", "person"), node("B", "person"), node("Zed", "person")];
        let mut doubtful = edge("A", "B", "related to");
        doubtful.confidence = confidence::INFERRED.to_string();
        let analysis = analyze(&nodes, &[doubtful]);
        assert_eq!(analysis.uncertain_edges.len(), 1);
        assert_eq!(analysis.disconnected, vec!["Zed".to_string()]);
    }

    #[test]
    fn questions_are_derived_from_signals() {
        let nodes = vec![node("A", "person"), node("B", "person"), node("C", "person")];
        let edges = vec![edge("A", "B", "r"), edge("B", "C", "r")];
        let analysis = analyze(&nodes, &edges);
        assert!(!analysis.suggested_questions.is_empty());
        assert!(analysis.suggested_questions[0].contains('B'));
    }

    #[test]
    fn neighbourhood_is_bounded_by_depth() {
        let labels = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];
        let edges = vec![
            edge("A", "B", "r"),
            edge("B", "C", "r"),
            edge("C", "D", "r"),
        ];
        let near = neighbourhood(&labels, &edges, &["A".to_string()], 1);
        assert_eq!(near, vec!["A".to_string(), "B".to_string()]);
    }
}
