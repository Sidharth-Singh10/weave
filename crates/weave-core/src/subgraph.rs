//! Bounded, relevant graph context for the extraction LLM.
//!
//! Picks the subgraph related to a new note: lexical anchors (exact label →
//! word overlap → substring), 1-hop neighborhood expansion (2-hop only while
//! budget remains), ranked before truncation, and hard node / edge / token
//! budgets. Nothing matches → an empty subgraph (the note is sent alone).
//!
//! Kept as its own module so V2/V3 can swap the retrieval strategy without
//! touching prompt construction.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::models::{GraphEdge, GraphNode};

/// Maximum number of matched nodes used as BFS seeds (independent entry
/// points into the graph). Matched nodes beyond this are still included at
/// hop 0; they just don't seed expansion.
pub const MAX_ANCHORS: usize = 8;
/// Hard safety bound on nodes included in the LLM context.
pub const MAX_NODES: usize = 64;
/// Hard safety bound on edges included in the LLM context.
pub const MAX_EDGES: usize = 256;
/// Maximum graph-traversal depth from an anchor.
pub const MAX_HOPS: usize = 2;
/// Estimated prompt-token ceiling for the graph context (labels + relations
/// chars ÷ 4). The final authority — node/edge caps alone can be defeated by
/// very long labels. Tune from admin-tracer data.
pub const ESTIMATED_TOKEN_CEILING: i64 = 1_500;

/// The selected subgraph plus the metadata the admin tracer surfaces.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubgraphSelection {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Matched labels used as BFS seeds.
    pub anchors: Vec<String>,
    pub subgraph_node_count: usize,
    pub subgraph_edge_count: usize,
    pub omitted_node_count: usize,
    pub omitted_edge_count: usize,
    /// Estimated prompt tokens of the selected context (chars ÷ 4).
    pub estimated_tokens: i64,
    /// Deepest hop actually included in the selection.
    pub max_hops: usize,
}

/// Match strength of a label against the note's candidate terms.
/// Higher wins; 0 = no match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchStrength {
    None = 0,
    Substring = 1,
    WordOverlap = 2,
    Exact = 3,
}

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "then", "else", "of", "at", "by", "for", "with",
    "without", "in", "on", "to", "from", "into", "about", "over", "under", "again", "further",
    "once", "here", "there", "when", "where", "why", "how", "all", "any", "both", "each", "few",
    "more", "most", "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so",
    "than", "too", "very", "can", "will", "just", "should", "now", "is", "are", "was", "were",
    "be", "been", "being", "do", "does", "did", "have", "has", "had", "having", "am", "it",
    "its", "this", "that", "these", "those", "you", "he", "she", "we", "they", "him", "her",
    "them", "your", "his", "their", "what", "which", "who", "whom", "also", "after", "before",
    "during", "because", "since", "while", "though", "through", "between", "among", "out",
    "get", "got", "gets", "make", "made", "makes", "say", "said", "says", "like", "used",
    "use", "uses", "using", "one", "two", "may", "might", "must", "could", "would", "shall",
    "upon", "via", "per", "vs", "etc",
];

/// Candidate terms from a note: proper-noun phrases plus significant
/// lowercase tokens (len >= 3, stopwords filtered). Deduplicated
/// case-insensitively, original order kept.
pub fn candidate_terms(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut push = |term: &str| {
        let t = term.trim();
        if t.is_empty() {
            return;
        }
        let key = t.to_lowercase();
        if seen.insert(key) {
            terms.push(t.to_string());
        }
    };

    let proper = regex::Regex::new(r"\b[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*\b").unwrap();
    for m in proper.find_iter(text) {
        push(m.as_str());
    }

    for token in text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'') {
        let t = token.trim().trim_matches(['-', '\'']);
        if t.len() < 3 {
            continue;
        }
        let lower = t.to_lowercase();
        if STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        // Proper nouns were already added above; add the rest as lowercase
        // tokens (keeps matching case-insensitive later anyway).
        push(t);
    }

    terms
}

fn words(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

/// Case-insensitive match strength of `label` against `terms`.
pub fn match_strength(label: &str, terms: &[String]) -> MatchStrength {
    let label_l = label.to_lowercase();
    let label_words: Vec<String> = words(&label_l).iter().map(|w| w.to_string()).collect();
    let mut best = MatchStrength::None;

    for term in terms {
        let term_l = term.to_lowercase();
        let strength = if label_l == term_l {
            MatchStrength::Exact
        } else {
            let term_words = words(&term_l);
            let word_overlap = term_words.iter().any(|tw| {
                label_words.iter().any(|lw| {
                    lw == tw
                        || lw.starts_with(&format!("{tw} "))
                        || tw.starts_with(&format!("{lw} "))
                })
            });
            // First-name style: "harry" resolves to "harry potter".
            let prefix = label_l.starts_with(&format!("{term_l} "))
                || term_l.starts_with(&format!("{label_l} "));
            if word_overlap || prefix {
                MatchStrength::WordOverlap
            } else if label_l.contains(&term_l) || term_l.contains(&label_l) {
                MatchStrength::Substring
            } else {
                MatchStrength::None
            }
        };
        if strength > best {
            best = strength;
        }
    }
    best
}

fn est_tokens(s: &str) -> i64 {
    ((s.chars().count() as i64) + 3) / 4
}

/// Degree of each label (case-insensitive) in the full graph.
fn degrees(nodes: &[GraphNode], edges: &[GraphEdge]) -> HashMap<String, usize> {
    let mut deg: HashMap<String, usize> = nodes
        .iter()
        .map(|n| (n.label.to_lowercase(), 0))
        .collect();
    for e in edges {
        *deg.entry(e.source_label.to_lowercase()).or_default() += 1;
        *deg.entry(e.target_label.to_lowercase()).or_default() += 1;
    }
    deg
}

/// Select the subgraph of `nodes`/`edges` relevant to `text`, subject to the
/// module budgets. Deterministic: ties break on lowercase label.
pub fn select_relevant_subgraph(
    text: &str,
    nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> SubgraphSelection {
    let terms = candidate_terms(text);

    let deg = degrees(nodes, edges);

    // Node key (lowercase label) -> full node + metadata.
    let mut node_by_key: HashMap<String, GraphNode> = HashMap::new();
    for n in nodes {
        node_by_key.entry(n.label.to_lowercase()).or_insert_with(|| n.clone());
    }
    // Preserve a stable ranking key order for deterministic iteration.
    let node_keys: Vec<String> = nodes
        .iter()
        .map(|n| n.label.to_lowercase())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Match strength per node; matched nodes are hop 0.
    let mut strength: HashMap<String, MatchStrength> = HashMap::new();
    for key in &node_keys {
        let label = &node_by_key[key].label;
        strength.insert(key.clone(), match_strength(label, &terms));
    }

    // Anchors: top MAX_ANCHORS matched nodes by (strength, degree) — the BFS
    // seeds. All matched nodes are selected at hop 0 regardless of the cap.
    let mut matched: Vec<&String> = node_keys
        .iter()
        .filter(|k| strength[*k] > MatchStrength::None)
        .collect();
    matched.sort_by(|a, b| {
        strength[*b]
            .cmp(&strength[*a])
            .then_with(|| deg.get(*b).unwrap_or(&0).cmp(deg.get(*a).unwrap_or(&0)))
            .then_with(|| a.cmp(b))
    });
    let seeds: Vec<String> = matched
        .iter()
        .take(MAX_ANCHORS)
        .map(|k| (*k).clone())
        .collect();

    // BFS hop distance from seeds, max MAX_HOPS deep, bidirectional.
    let mut hop: HashMap<String, usize> = HashMap::new();
    for k in &matched {
        hop.insert((*k).clone(), 0);
    }
    if !seeds.is_empty() {
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for e in edges {
            adj.entry(e.source_label.to_lowercase())
                .or_default()
                .push(e.target_label.to_lowercase());
            adj.entry(e.target_label.to_lowercase())
                .or_default()
                .push(e.source_label.to_lowercase());
        }
        let mut queue: VecDeque<String> = seeds.iter().cloned().collect();
        while let Some(cur) = queue.pop_front() {
            let d = hop[&cur];
            if d >= MAX_HOPS {
                continue;
            }
            if let Some(neighbors) = adj.get(&cur) {
                for n in neighbors.clone() {
                    let nd = d + 1;
                    if node_by_key.contains_key(&n) {
                        match hop.get(&n) {
                            Some(existing) if *existing <= nd => {}
                            _ => {
                                hop.insert(n.clone(), nd);
                                queue.push_back(n);
                            }
                        }
                    }
                }
            }
        }
    }

    // Rank candidates: hop asc → strength desc → degree desc → label asc.
    let mut ranked: Vec<&String> = node_keys
        .iter()
        .filter(|k| hop.contains_key(*k))
        .collect();
    ranked.sort_by(|a, b| {
        hop[*a]
            .cmp(&hop[*b])
            .then_with(|| {
                strength
                    .get(*b)
                    .unwrap_or(&MatchStrength::None)
                    .cmp(strength.get(*a).unwrap_or(&MatchStrength::None))
            })
            .then_with(|| deg.get(*b).unwrap_or(&0).cmp(deg.get(*a).unwrap_or(&0)))
            .then_with(|| a.cmp(b))
    });

    // Greedy node selection under node + token budgets.
    let mut sel_keys: HashSet<String> = HashSet::new();
    let mut selected_hop = 0usize;
    let mut token_budget = ESTIMATED_TOKEN_CEILING;
    for key in ranked {
        if sel_keys.len() >= MAX_NODES {
            break;
        }
        let label = &node_by_key[key].label;
        let cost = est_tokens(label);
        if cost > token_budget {
            break;
        }
        token_budget -= cost;
        selected_hop = selected_hop.max(hop[key]);
        sel_keys.insert(key.clone());
    }

    // Rank edges by endpoint hop sum asc → endpoint strength sum desc.
    let edge_score = |e: &GraphEdge| -> (usize, u8, String) {
        let a = e.source_label.to_lowercase();
        let b = e.target_label.to_lowercase();
        let hop_sum = hop.get(&a).copied().unwrap_or(usize::MAX)
            + hop.get(&b).copied().unwrap_or(usize::MAX);
        let s = (*strength.get(&a).unwrap_or(&MatchStrength::None)) as u8
            + (*strength.get(&b).unwrap_or(&MatchStrength::None)) as u8;
        (hop_sum, s, format!("{a}|{b}|{}", e.relation))
    };
    let mut ranked_edges: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| {
            sel_keys.contains(&e.source_label.to_lowercase())
                && sel_keys.contains(&e.target_label.to_lowercase())
        })
        .collect();
    ranked_edges.sort_by_key(|e| edge_score(e));

    let mut selected_edge_keys: HashSet<(String, String, String)> = HashSet::new();
    for e in ranked_edges {
        if selected_edge_keys.len() >= MAX_EDGES {
            break;
        }
        let cost = est_tokens(&format!(
            "{} {} {}",
            e.source_label, e.relation, e.target_label
        ));
        if cost > token_budget {
            break;
        }
        token_budget -= cost;
        selected_edge_keys.insert(edge_key(e));
    }

    // Preserve the original graph order for stable prompts.
    let sel_nodes: Vec<GraphNode> = nodes
        .iter()
        .filter(|n| sel_keys.contains(&n.label.to_lowercase()))
        .cloned()
        .collect();
    let sel_edges: Vec<GraphEdge> = edges
        .iter()
        .filter(|e| selected_edge_keys.contains(&edge_key(e)))
        .cloned()
        .collect();
    let node_count = sel_nodes.len();
    let edge_count = sel_edges.len();

    SubgraphSelection {
        nodes: sel_nodes,
        edges: sel_edges,
        anchors: seeds,
        subgraph_node_count: node_count,
        subgraph_edge_count: edge_count,
        omitted_node_count: nodes.len().saturating_sub(node_count),
        omitted_edge_count: edges.len().saturating_sub(edge_count),
        estimated_tokens: ESTIMATED_TOKEN_CEILING - token_budget,
        max_hops: selected_hop,
    }
}

fn edge_key(e: &GraphEdge) -> (String, String, String) {
    (
        e.source_label.to_lowercase(),
        e.target_label.to_lowercase(),
        e.relation.to_lowercase(),
    )
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

    fn labels(sel: &SubgraphSelection) -> Vec<String> {
        let mut v: Vec<String> = sel.nodes.iter().map(|n| n.label.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn terms_extract_proper_nouns_and_significant_tokens() {
        let terms = candidate_terms("Harry Potter studies at Hogwarts with his owl.");
        let lower: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
        assert!(lower.contains(&"harry potter".to_string()));
        assert!(lower.contains(&"hogwarts".to_string()));
        assert!(lower.contains(&"studies".to_string()));
        assert!(lower.contains(&"owl".to_string()));
        assert!(!lower.contains(&"with".to_string()), "stopwords excluded");
        assert!(!lower.contains(&"his".to_string()), "stopwords excluded");
    }

    #[test]
    fn match_strength_precedence() {
        let terms = vec!["harry potter".to_string()];
        assert_eq!(match_strength("harry potter", &terms), MatchStrength::Exact);
        assert_eq!(match_strength("Harry", &terms), MatchStrength::WordOverlap);
        assert_eq!(match_strength("potter", &terms), MatchStrength::WordOverlap);
        // Stem/plural overlap: substring only, no word-level match.
        assert_eq!(
            match_strength("Riddle", &["riddles".to_string()]),
            MatchStrength::Substring
        );
        assert_eq!(match_strength("Hermione", &terms), MatchStrength::None);
    }

    #[test]
    fn unrelated_isolated_node_is_omitted() {
        let nodes = vec![node("Harry Potter"), node("Quantum Physics")];
        let edges = vec![];
        let sel = select_relevant_subgraph("Harry studies at Hogwarts.", &nodes, &edges);
        assert_eq!(labels(&sel), vec!["Harry Potter"]);
        assert_eq!(sel.omitted_node_count, 1);
        assert!(sel.anchors.contains(&"harry potter".to_string()));
    }

    #[test]
    fn two_hop_neighbor_included_when_budget_permits() {
        // Harry -[friend of]-> Ron -[friend of]-> Hermione. Note mentions Harry.
        let nodes = vec![
            node("Harry Potter"),
            node("Ron"),
            node("Hermione"),
        ];
        let edges = vec![
            edge("Harry Potter", "Ron", "friend of"),
            edge("Ron", "Hermione", "friend of"),
        ];
        let sel = select_relevant_subgraph("Harry likes quidditch.", &nodes, &edges);
        let l = labels(&sel);
        assert!(l.contains(&"Harry Potter".to_string()));
        assert!(l.contains(&"Ron".to_string()), "1-hop must be included");
        assert!(l.contains(&"Hermione".to_string()), "2-hop fits the budget");
        assert_eq!(sel.max_hops, 2);
        assert_eq!(sel.subgraph_edge_count, 2);
    }

    #[test]
    fn empty_match_yields_empty_subgraph() {
        let nodes = vec![node("Harry Potter"), node("Hogwarts")];
        let edges = vec![edge("Harry Potter", "Hogwarts", "studies at")];
        let sel = select_relevant_subgraph("Completely unrelated topic.", &nodes, &edges);
        assert!(sel.nodes.is_empty());
        assert!(sel.edges.is_empty());
        assert_eq!(sel.omitted_node_count, 2);
        assert_eq!(sel.omitted_edge_count, 1);
        assert!(sel.anchors.is_empty());
        assert_eq!(sel.max_hops, 0);
    }

    #[test]
    fn high_degree_hub_cannot_consume_budget() {
        // Hub "X" connects to 300 leaf nodes; note mentions a single leaf.
        let mut nodes = vec![node("X"), node("Mentioned")];
        let mut edges = vec![edge("X", "Mentioned", "connected to")];
        for i in 0..300 {
            nodes.push(node(&format!("Leaf {i}")));
            edges.push(edge("X", &format!("Leaf {i}"), "connected to"));
        }
        let sel = select_relevant_subgraph("Mentioned is special.", &nodes, &edges);
        assert!(sel.subgraph_node_count <= MAX_NODES);
        let l = labels(&sel);
        assert!(l.contains(&"Mentioned".to_string()));
        assert!(l.contains(&"X".to_string()));
        // Leaves are hop 2 from the anchor: only included while budget lasts.
        assert!(sel.subgraph_node_count < 64 || sel.subgraph_edge_count <= MAX_EDGES);
    }

    #[test]
    fn token_ceiling_evicts_low_ranked_first() {
        // Many matched nodes with long labels: token budget is the binding cap.
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for i in 0..40 {
            let label = format!("Very Long Concept Label Number {i}");
            nodes.push(node(&label));
            if i > 0 {
                edges.push(edge(
                    "Very Long Concept Label Number 0",
                    &label,
                    "related to",
                ));
            }
        }
        let sel = select_relevant_subgraph(
            "Very Long Concept Label Number 0 appears here.",
            &nodes,
            &edges,
        );
        assert!(sel.estimated_tokens <= ESTIMATED_TOKEN_CEILING);
        assert!(sel.subgraph_node_count <= MAX_NODES);
        assert!(sel.subgraph_edge_count <= MAX_EDGES);
        // The exact-match anchor is included.
        assert!(labels(&sel).contains(&"Very Long Concept Label Number 0".to_string()));
    }

    #[test]
    fn matched_nodes_beyond_anchor_cap_are_still_included() {
        let mut nodes = Vec::new();
        for i in 0..12 {
            nodes.push(node(&format!("Concept {i}")));
        }
        let text = (0..12)
            .map(|i| format!("Concept {i}"))
            .collect::<Vec<_>>()
            .join(" and ");
        let sel = select_relevant_subgraph(&text, &nodes, &[]);
        assert_eq!(sel.anchors.len(), MAX_ANCHORS, "seeds capped at 8");
        assert_eq!(
            sel.subgraph_node_count, 12,
            "all matched nodes included regardless of anchor cap"
        );
    }

    #[test]
    fn substring_compatibility_with_mcp_style_matching() {
        // MCP matches by substring (strpos). The note contains "harry potter
        // and hermione" while labels are "Harry" and "Hermione" — substring
        // must find them.
        let nodes = vec![node("Harry"), node("Hermione"), node("Ron")];
        let sel = select_relevant_subgraph("Harry Potter and Hermione are friends.", &nodes, &[]);
        let l = labels(&sel);
        assert!(l.contains(&"Harry".to_string()));
        assert!(l.contains(&"Hermione".to_string()));
        assert!(!l.contains(&"Ron".to_string()));
    }

    #[test]
    fn hops_limited_to_one_when_budget_tight() {
        // Anchor has 70 hop-1 neighbors (plus one neighbor reaching hop-2
        // "Far"). Under the 64-node cap, hop-1 fills the budget, so hop-2 is
        // never reached.
        let mut nodes = vec![node("Anchor"), node("Neighbor 0"), node("Far")];
        let mut edges = vec![
            edge("Anchor", "Neighbor 0", "near"),
            edge("Neighbor 0", "Far", "distant"),
        ];
        for i in 1..70 {
            nodes.push(node(&format!("Neighbor {i}")));
            edges.push(edge("Anchor", &format!("Neighbor {i}"), "near"));
        }
        let sel = select_relevant_subgraph("Anchor is central.", &nodes, &edges);
        let l = labels(&sel);
        assert!(l.contains(&"Anchor".to_string()));
        assert!(
            !l.contains(&"Far".to_string()),
            "hop-2 must be excluded when 1-hop fills the budget"
        );
        assert_eq!(sel.max_hops, 1);
        assert_eq!(sel.subgraph_node_count, MAX_NODES);
    }
}