use regex::Regex;

use crate::llm::OpenCodeClient;
use crate::models::{GraphDelta, GraphEdge, GraphNode, IngestRequest};

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

pub async fn extract_delta(client: &OpenCodeClient, req: &IngestRequest) -> GraphDelta {
    if client.available() {
        match extract_with_llm(client, req).await {
            Ok(delta) => return delta,
            Err(e) => {
                tracing::warn!("LLM extraction failed, falling back to mock: {e}");
            }
        }
    }
    extract_mock(req)
}

async fn extract_with_llm(client: &OpenCodeClient, req: &IngestRequest) -> anyhow::Result<GraphDelta> {
    let existing: Vec<String> = req.nodes.iter().map(|n| n.label.clone()).collect();
    let existing_edges: Vec<String> = req
        .edges
        .iter()
        .map(|e| format!("{} -[{}]-> {}", e.source_label, e.relation, e.target_label))
        .collect();
    let user = format!(
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
    );

    let json = client.chat_json(SYSTEM_PROMPT, &user).await?;
    let mut delta: GraphDelta = serde_json::from_value(json)?;
    dedup_against_existing(&mut delta, &existing);
    Ok(delta)
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
                label: e.clone(),
                kind: guess_kind(&relation, e, is_subject),
            });
        }
    }

    // Edges: subject -> every other entity (relation between them).
    for target in entities.iter().skip(1) {
        if !target.eq_ignore_ascii_case(&subject) {
            delta.edges.push(GraphEdge {
                source_label: subject.clone(),
                target_label: target.clone(),
                relation: relation.clone(),
            });
        }
    }
    // Edges: subject -> each captured lowercase object.
    for o in &objects {
        delta.edges.push(GraphEdge {
            source_label: subject.clone(),
            target_label: o.clone(),
            relation: relation.clone(),
        });
    }

    dedup_against_existing(&mut delta, &existing);
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
