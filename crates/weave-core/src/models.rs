use serde::{Deserialize, Serialize};

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
