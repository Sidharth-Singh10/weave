use serde::{Deserialize, Serialize};

/// How an edge's relationship is grounded in the note that produced it.
///
/// Mirrors the three-class vocabulary used by deterministic graph builders:
/// - `EXTRACTED` — the relation is stated in the source note.
/// - `INFERRED` — the relation is a reasonable deduction (generic relation,
///   only part of the wording present, or resolved through graph context).
/// - `AMBIGUOUS` — neither endpoint nor relation wording appears in the note,
///   so the connection is a model guess that a human should review.
pub mod confidence {
    pub const EXTRACTED: &str = "EXTRACTED";
    pub const INFERRED: &str = "INFERRED";
    pub const AMBIGUOUS: &str = "AMBIGUOUS";
}

fn default_confidence() -> String {
    confidence::EXTRACTED.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    #[serde(default)]
    pub id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    pub source_label: String,
    pub target_label: String,
    pub relation: String,
    /// `EXTRACTED` / `INFERRED` / `AMBIGUOUS` — see [`confidence`].
    #[serde(default = "default_confidence")]
    pub confidence: String,
    /// The sentence from the source note that supports this edge, when one
    /// could be located deterministically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

impl Default for GraphEdge {
    fn default() -> Self {
        Self {
            id: None,
            source_id: None,
            target_id: None,
            source_label: String::new(),
            target_label: String::new(),
            relation: String::new(),
            confidence: default_confidence(),
            evidence: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub text: String,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphDelta {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

// --- AI-assisted organization -------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OrganizeRequest {
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptGroup {
    pub label: String,
    #[serde(default)]
    pub member_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatePair {
    pub label_a: String,
    pub label_b: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OrganizeResult {
    #[serde(default)]
    pub groups: Vec<ConceptGroup>,
    #[serde(default)]
    pub missing_edges: Vec<GraphEdge>,
    #[serde(default)]
    pub disconnected: Vec<String>,
    #[serde(default)]
    pub duplicates: Vec<DuplicatePair>,
}

// --- Semantic search ----------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub matches: Vec<String>,
    pub rationale: String,
}

// --- AI community labeling --------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LabelCommunityRequest {
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LabelCommunityResult {
    pub label: String,
}
