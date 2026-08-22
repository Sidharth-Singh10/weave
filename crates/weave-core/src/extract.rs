use std::collections::{HashMap, HashSet};

use regex::Regex;

use crate::llm::{OpenCodeClient, TokenUsage};
use crate::models::{GraphDelta, GraphEdge, GraphNode, IngestRequest};

/// Stable, human-readable node IDs: `node-{slug}`, with a numeric suffix on collision.
fn slug(label: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "node".to_string()
    } else {
        out
    }
}

fn unique_node_id(label: &str, used: &HashSet<String>) -> String {
    let base = format!("node-{}", slug(label));
    if !used.contains(&base) {
        return base;
    }
    let mut i = 2;
    loop {
        let candidate = format!("{base}-{i}");
        if !used.contains(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

/// Translation layer between LLM label output and stable IDs.
/// Runs after deduplication: edge labels are already canonical.
/// Existing nodes keep their request IDs; new nodes receive generated IDs.
fn assign_ids(delta: &mut GraphDelta, existing_nodes: &[GraphNode]) {
    let mut used: HashSet<String> = existing_nodes.iter().filter_map(|n| n.id.clone()).collect();
    let mut id_for_label: HashMap<String, String> = HashMap::new();
    for n in existing_nodes {
        if let Some(id) = &n.id {
            id_for_label
                .entry(n.label.to_lowercase())
                .or_insert_with(|| id.clone());
        }
    }

    for node in &mut delta.nodes {
        let key = node.label.to_lowercase();
        let id = if let Some(existing_id) = id_for_label.get(&key) {
            existing_id.clone()
        } else {
            let new_id = unique_node_id(&node.label, &used);
            used.insert(new_id.clone());
            id_for_label.insert(key, new_id.clone());
            new_id
        };
        node.id = Some(id);
    }

    for edge in &mut delta.edges {
        edge.source_id = id_for_label.get(&edge.source_label.to_lowercase()).cloned();
        edge.target_id = id_for_label.get(&edge.target_label.to_lowercase()).cloned();
    }
}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn slug_normalizes_labels() {
        assert_eq!(slug("Harry Potter"), "harry-potter");
        assert_eq!(slug("Hogwarts School"), "hogwarts-school");
        assert_eq!(slug("C++"), "c");
        assert_eq!(slug("   "), "node");
    }

    #[test]
    fn unique_node_id_handles_collisions() {
        let mut used: HashSet<String> = ["node-harry".to_string()].into();
        assert_eq!(unique_node_id("Harry", &used), "node-harry-2");
        used.insert("node-harry-2".to_string());
        assert_eq!(unique_node_id("Harry", &used), "node-harry-3");
        assert_eq!(unique_node_id("Hermione", &used), "node-hermione");
    }

    #[test]
    fn assign_ids_resolves_existing_and_new() {
        let existing = vec![GraphNode {
            id: Some("node-harry-potter".to_string()),
            label: "Harry Potter".to_string(),
            kind: "person".to_string(),
        }];
        let mut delta = GraphDelta {
            nodes: vec![
                GraphNode {
                    id: None,
                    label: "Hogwarts".to_string(),
                    kind: "place".to_string(),
                },
            ],
            edges: vec![GraphEdge {
                id: None,
                source_id: None,
                target_id: None,
                source_label: "Harry Potter".to_string(),
                target_label: "Hogwarts".to_string(),
                relation: "studies at".to_string(),
            }],
        };
        assign_ids(&mut delta, &existing);
        assert_eq!(delta.nodes[0].id.as_deref(), Some("node-hogwarts"));
        assert_eq!(delta.edges[0].source_id.as_deref(), Some("node-harry-potter"));
        assert_eq!(delta.edges[0].target_id.as_deref(), Some("node-hogwarts"));
    }
}

const SYSTEM_PROMPT: &str = r#"You are a knowledge-graph extractor for a note-taking app.
The user types natural-language notes. You extract concepts and relationships.

Rules:
- Every important concept (person, place, organization, event, object, topic) becomes a node.
- Every relationship becomes an edge with a short lowercase relation label (e.g. "friend of", "studies at", "part of", "located in").
- Reuse existing node labels EXACTLY as given when referring to the same concept (case-insensitive match is fine, but return the existing label spelling). Never duplicate an existing concept.
- Only return NEW nodes and NEW edges. If the input adds nothing new, return empty arrays.
- Keep labels short (1-4 words). Use title case for proper nouns, lowercase for generic concepts.
- Respond with strict JSON only, matching: {"nodes":[{"label":"...","kind":"person|place|org|event|object|concept"}],"edges":[{"source_label":"...","target_label":"...","relation":"..."}]}
"#;

/// Everything the pipeline tracer needs to show how one ingest reached the
/// LLM and what came back.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractTrace {
    /// The system prompt sent as the `system` message.
    pub system_prompt: String,
    /// The user message: existing node labels + edges + the new note.
    pub user_prompt: String,
    /// True when the deterministic mock extractor was used instead of the LLM.
    pub mock_fallback: bool,
    /// The full request body posted to `/chat/completions` (when LLM used).
    pub llm_request: Option<serde_json::Value>,
    /// The raw provider response text (when LLM used).
    pub llm_raw_response: Option<String>,
    /// The parsed JSON the model produced (when LLM used).
    pub llm_json: Option<serde_json::Value>,
    /// Provider-reported token usage (when LLM used).
    pub usage: Option<TokenUsage>,
}

pub async fn extract_delta(
    client: &OpenCodeClient,
    req: &IngestRequest,
) -> (GraphDelta, Option<TokenUsage>) {
    let (delta, usage, _trace) = extract_delta_traced(client, req).await;
    (delta, usage)
}

/// The real extraction pipeline plus a full trace of the LLM interaction.
/// `extract_delta` shares this implementation, so the trace reflects exactly
/// what a live ingest sends and receives.
pub async fn extract_delta_traced(
    client: &OpenCodeClient,
    req: &IngestRequest,
) -> (GraphDelta, Option<TokenUsage>, ExtractTrace) {
    if client.available() {
        match extract_with_llm(client, req).await {
            Ok((delta, trace)) => return (delta, trace.usage, trace),
            Err(e) => {
                tracing::warn!("LLM extraction failed, falling back to mock: {e}");
            }
        }
    }
    let mock_trace = ExtractTrace {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_prompt: build_ingest_user_prompt(req),
        mock_fallback: true,
        llm_request: None,
        llm_raw_response: None,
        llm_json: None,
        usage: None,
    };
    (extract_mock(req), None, mock_trace)
}

/// The exact user message sent to the LLM on ingest: the existing graph
/// (node labels + edges) followed by the new note.
pub fn build_ingest_user_prompt(req: &IngestRequest) -> String {
    let existing: Vec<String> = req.nodes.iter().map(|n| n.label.clone()).collect();
    let existing_edges: Vec<String> = req
        .edges
        .iter()
        .map(|e| format!("{} -[{}]-> {}", e.source_label, e.relation, e.target_label))
        .collect();
    format!(
        "Existing node labels: {}\nExisting edges: {}\n\nNew note: {}",
        if existing.is_empty() {
            "(none)".to_string()
        } else {
            existing.join(", ")
        },
        if existing_edges.is_empty() {
            "(none)".to_string()
        } else {
            existing_edges.join("; ")
        },
        req.text
    )
}

async fn extract_with_llm(
    client: &OpenCodeClient,
    req: &IngestRequest,
) -> anyhow::Result<(GraphDelta, ExtractTrace)> {
    let user = build_ingest_user_prompt(req);

    let out = client.chat_json_traced(SYSTEM_PROMPT, &user).await?;
    let mut delta: GraphDelta = serde_json::from_value(out.json.clone())?;
    dedup_against_existing(&mut delta, &req.nodes.iter().map(|n| n.label.clone()).collect::<Vec<_>>());
    assign_ids(&mut delta, &req.nodes);
    let trace = ExtractTrace {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_prompt: user,
        mock_fallback: false,
        llm_request: Some(out.trace.request_body),
        llm_raw_response: Some(out.trace.raw_response),
        llm_json: Some(out.json),
        usage: out.usage,
    };
    Ok((delta, trace))
}

/// Case-insensitive match, plus first-name / prefix matching so
/// "Harry" resolves to an existing "Harry Potter".
fn labels_match(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    a.starts_with(&format!("{b} ")) || b.starts_with(&format!("{a} "))
}

/// Drop nodes that already exist (case-insensitive) and rewrite edge
/// endpoints to the canonical existing label where possible.
fn dedup_against_existing(delta: &mut GraphDelta, existing: &[String]) {
    let canon = |label: &str| -> String {
        existing
            .iter()
            .find(|e| labels_match(e, label))
            .cloned()
            .unwrap_or_else(|| label.to_string())
    };

    delta
        .nodes
        .retain(|n| !existing.iter().any(|e| labels_match(e, &n.label)));
    delta.nodes.sort_by(|a, b| a.label.cmp(&b.label));
    delta.nodes.dedup_by(|a, b| labels_match(&a.label, &b.label));

    for edge in &mut delta.edges {
        edge.source_label = canon(&edge.source_label);
        edge.target_label = canon(&edge.target_label);
    }
    delta.edges.dedup_by(|a, b| {
        labels_match(&a.source_label, &b.source_label)
            && labels_match(&a.target_label, &b.target_label)
            && a.relation.eq_ignore_ascii_case(&b.relation)
    });
}

// ---------------------------------------------------------------------------
// Deterministic mock extractor (no API key). Handles simple Subject-Verb-Object
// notes like the Harry Potter demo flow. Not general-purpose NLP.
// ---------------------------------------------------------------------------

fn extract_mock(req: &IngestRequest) -> GraphDelta {
    let existing: Vec<String> = req.nodes.iter().map(|n| n.label.clone()).collect();
    let text = req.text.trim().trim_end_matches(['.', '!', '?']);
    let mut delta = GraphDelta {
        nodes: vec![],
        edges: vec![],
    };

    if text.is_empty() {
        return delta;
    }

    let proper = Regex::new(r"\b[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*\b").unwrap();
    let candidates: Vec<String> = proper
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect();

    // Split lists like "Ron and Hermione" into separate entities.
    let mut entities: Vec<String> = vec![];
    for c in candidates {
        for part in c.split(" and ") {
            let p = part.trim();
            if !p.is_empty() {
                entities.push(p.to_string());
            }
        }
    }
    entities.dedup();

    // Subject = first entity mentioned (often possessive like "Harry's").
    let subject = entities.first().cloned().unwrap_or_else(|| text.to_string());
    let lower = text.to_lowercase();

    // Relation patterns: (verb phrase, canonical relation).
    let patterns: &[(&str, &str)] = &[
        ("best friend", "friend of"),
        ("friend", "friend of"),
        ("studies at", "studies at"),
        ("study at", "studies at"),
        ("works at", "works at"),
        ("work at", "works at"),
        ("located in", "located in"),
        ("lives in", "lives in"),
        ("afraid of", "afraid of"),
        ("part of", "part of"),
        ("invented by", "invented by"),
        ("parent of", "parent of"),
        ("has", "has"),
        ("have", "has"),
        ("is a", "is a"),
        ("is an", "is a"),
        ("is", "related to"),
        ("are", "related to"),
    ];

    let relation = patterns
        .iter()
        .find(|(phrase, _)| lower.contains(phrase))
        .map(|(_, rel)| rel.to_string())
        .unwrap_or_else(|| "related to".to_string());

    // Lowercase objects after the LAST relation phrase become concept nodes,
    // e.g. "Ron is afraid of spiders." -> "spiders",
    // "Hogwarts has four houses." -> "houses".
    // Using the last occurrence avoids leftovers when an earlier phrase
    // ("best friend") is followed by a copula ("... are Ron and Hermione").
    let mut objects: Vec<String> = vec![];
    let last_match = patterns
        .iter()
        .filter_map(|(phrase, _)| lower.find(phrase).map(|pos| (pos, *phrase)))
        .max_by_key(|(pos, _)| *pos);
    if let Some((pos, matched)) = last_match {
        let after = &lower[pos + matched.len()..];
        for part in after.split(" and ").chain(after.split(',')).collect::<Vec<_>>() {
            let o = clean_object(part);
            if !o.is_empty()
                && !entities
                    .iter()
                    .any(|e| labels_match(e, &o) || o.eq_ignore_ascii_case(e))
                && !objects.iter().any(|x| x.eq_ignore_ascii_case(&o))
            {
                objects.push(o);
            }
        }
    }

    // New nodes: entities (subject + named objects) and captured lowercase objects.
    for e in entities.iter().chain(objects.iter()) {
        if !existing.iter().any(|x| labels_match(x, e))
            && !delta.nodes.iter().any(|n| labels_match(&n.label, e))
        {
            let is_subject = e.eq_ignore_ascii_case(&subject);
            delta.nodes.push(GraphNode {
                id: None,
                label: e.clone(),
                kind: guess_kind(&relation, e, is_subject),
            });
        }
    }

    // Edges: subject -> every other entity (relation between them).
    for target in entities.iter().skip(1) {
        if !target.eq_ignore_ascii_case(&subject) {
            delta.edges.push(GraphEdge {
                id: None,
                source_id: None,
                target_id: None,
                source_label: subject.clone(),
                target_label: target.clone(),
                relation: relation.clone(),
            });
        }
    }
    // Edges: subject -> each captured lowercase object.
    for o in &objects {
        delta.edges.push(GraphEdge {
            id: None,
            source_id: None,
            target_id: None,
            source_label: subject.clone(),
            target_label: o.clone(),
            relation: relation.clone(),
        });
    }

    dedup_against_existing(&mut delta, &existing);
    assign_ids(&mut delta, &req.nodes);
    delta
}

/// Normalize a lowercase object phrase: strip leading articles and
/// quantifiers ("a", "the", "four", "two"), trailing possessives, and
/// empty tokens. Keeps the noun itself.
fn clean_object(part: &str) -> String {
    let mut o = part.trim().to_string();
    let articles: &[&str] = &["a ", "an ", "the "];
    for a in articles {
        if let Some(rest) = o.strip_prefix(a) {
            o = rest.to_string();
            break;
        }
    }
    if let Some(stripped) = o.strip_prefix(|c: char| c.is_ascii_digit()) {
        o = stripped.trim_start().to_string();
    }
    // "four houses" -> "houses"
    const QUANT: [&str; 8] = ["two ", "three ", "four ", "five ", "six ", "seven ", "eight ", "nine "];
    for q in QUANT {
        if let Some(rest) = o.strip_prefix(q) {
            o = rest.to_string();
            break;
        }
    }
    if let Some(stripped) = o.strip_suffix("'s") {
        o = stripped.to_string();
    }
    o.trim().to_string()
}

fn guess_kind(relation: &str, label: &str, is_subject: bool) -> String {
    let titled = label
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);
    if is_subject && titled {
        return "person".into();
    }
    match relation {
        "friend of" | "parent of" => "person".into(),
        "studies at" | "works at" => "org".into(),
        "located in" | "lives in" => "place".into(),
        "has" | "is a" => "concept".into(),
        _ if titled => "person".into(),
        _ => "concept".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(text: &str, nodes: &[&str]) -> IngestRequest {
        IngestRequest {
            text: text.to_string(),
            nodes: nodes
                .iter()
                .map(|l| GraphNode {
                    id: None,
                    label: l.to_string(),
                    kind: String::new(),
                })
                .collect(),
            edges: vec![],
        }
    }

    #[test]
    fn single_concept_creates_central_node() {
        let delta = extract_mock(&req("Harry Potter", &[]));
        assert_eq!(delta.nodes.len(), 1);
        assert_eq!(delta.nodes[0].label, "Harry Potter");
        assert!(delta.edges.is_empty());
    }

    #[test]
    fn friends_list_detects_all_relationships() {
        let delta = extract_mock(&req("Harry's best friends are Ron and Hermione.", &["Harry Potter"]));
        let labels: Vec<&str> = delta.nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["Hermione", "Ron"]); // "Harry" prefix-matches existing "Harry Potter"
        assert_eq!(delta.edges.len(), 2);
        assert!(delta.edges.iter().all(|e| e.source_label == "Harry Potter"));
        assert!(delta.edges.iter().all(|e| e.relation == "friend of"));
    }

    #[test]
    fn dedups_prefix_and_lowercase_names() {
        let delta = extract_mock(&req("Harry studies at Hogwarts.", &["Harry Potter"]));
        assert_eq!(delta.nodes.len(), 1);
        assert_eq!(delta.nodes[0].label, "Hogwarts");
        assert_eq!(delta.edges[0].source_label, "Harry Potter");
        assert_eq!(delta.edges[0].relation, "studies at");
    }

    #[test]
    fn captures_lowercase_objects() {
        let delta = extract_mock(&req("Ron is afraid of spiders.", &["Ron"]));
        assert_eq!(delta.nodes.len(), 1);
        assert_eq!(delta.nodes[0].label, "spiders");
        assert_eq!(delta.edges[0].target_label, "spiders");
        assert_eq!(delta.edges[0].relation, "afraid of");
    }

    #[test]
    fn strips_quantifiers_from_objects() {
        let delta = extract_mock(&req("Hogwarts has four houses.", &["Hogwarts"]));
        assert_eq!(delta.nodes.len(), 1);
        assert_eq!(delta.nodes[0].label, "houses");
        assert_eq!(delta.edges[0].relation, "has");
    }

    #[test]
    fn labels_match_prefix() {
        assert!(labels_match("Harry", "Harry Potter"));
        assert!(labels_match("Harry Potter", "Harry"));
        assert!(labels_match("HOGWARTS", "hogwarts"));
        assert!(!labels_match("Harry", "Hermione"));
    }

    #[test]
    fn clean_object_strips_articles_and_quantifiers() {
        assert_eq!(clean_object("four houses"), "houses");
        assert_eq!(clean_object("a wizard"), "wizard");
        assert_eq!(clean_object("the spiders"), "spiders");
        assert_eq!(clean_object("three 4th year students"), "4th year students");
    }
}
