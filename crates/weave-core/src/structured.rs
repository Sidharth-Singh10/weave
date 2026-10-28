//! Deterministic extraction for notes that already state their graph.
//!
//! An LLM is the right tool for prose and the wrong tool for a note that
//! literally lists entities and relationships: it costs tokens, varies between
//! runs, and can only re-derive what was already written down. This module
//! parses two explicit formats instead:
//!
//! - **Markdown outlines** — a heading owns the links beneath it:
//!
//!   ```markdown
//!   # Harry Potter
//!   A wizard.
//!
//!   ## Relationships
//!   - [[Hogwarts]] — studies at
//!   - [[Ron Weasley]] — friend of
//!   ```
//!
//!   A link with no stated relation becomes `links to`; `[[Note|alias]]` and
//!   `[[Note#section]]` resolve to `Note`.
//!
//! - **JSON** — `{ "nodes": [{ "label", "kind" }], "edges": [{ "source",
//!   "target", "relation" }] }`, or an array of nodes with nested
//!   `relations`.
//!
//! Both are exact, so every edge is `EXTRACTED` (or `INFERRED` for a generic
//! relation such as "related to"), with the source line kept as evidence.
//! Everything else returns `None` so the LLM path still runs: a note that is
//! mostly prose is not hijacked just because it contains one heading.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::extract::is_generic_relation;
use crate::models::{confidence, GraphDelta, GraphEdge, GraphNode};

pub const FORMAT_MARKDOWN: &str = "markdown";
pub const FORMAT_JSON: &str = "json";

const DEFAULT_KIND: &str = "concept";
/// Relation used when a note links to a concept without saying how.
const MENTION_RELATION: &str = "links to";
const MAX_EVIDENCE_CHARS: usize = 280;
/// A note must be at least this structured (headings + bullets) to be parsed
/// here; below the threshold it is prose and belongs to the LLM.
const STRUCTURED_RATIO_PERCENT: usize = 60;

pub struct StructuredExtraction {
    /// [`FORMAT_MARKDOWN`] or [`FORMAT_JSON`], for the pipeline trace.
    pub format: &'static str,
    pub delta: GraphDelta,
}

/// Parse a note that states its own graph, or `None` if it does not.
pub fn extract(text: &str) -> Option<StructuredExtraction> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(delta) = from_json_note(trimmed) {
        return Some(StructuredExtraction {
            format: FORMAT_JSON,
            delta,
        });
    }

    from_markdown(text).map(|delta| StructuredExtraction {
        format: FORMAT_MARKDOWN,
        delta,
    })
}

/// JSON is only attempted when the note clearly starts as one; a failed parse
/// simply falls through to Markdown and then to the LLM.
fn from_json_note(text: &str) -> Option<GraphDelta> {
    if !(text.starts_with('{') || text.starts_with('[')) {
        return None;
    }
    from_json(text)
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

fn from_markdown(text: &str) -> Option<GraphDelta> {
    // The shallowest heading level names entities; deeper headings are section
    // labels ("## Relationships"), so links beneath them stay attached to the
    // entity above.
    let subject_level = subject_heading_level(text)?;

    let mut subjects: Vec<String> = Vec::new();
    let mut subject_index: HashMap<String, usize> = HashMap::new();
    let mut current: Option<usize> = None;

    let mut node_labels: Vec<String> = Vec::new();
    let mut node_index: HashMap<String, usize> = HashMap::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut edge_keys: HashSet<String> = HashSet::new();

    let mut headings = 0usize;
    let mut graph_lines = 0usize;
    let mut prose = 0usize;
    let mut fenced = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced || line.is_empty() {
            continue;
        }

        if let Some((level, title)) = heading(line) {
            headings += 1;
            if level == subject_level {
                let index = *subject_index.entry(title.to_lowercase()).or_insert_with(|| {
                    subjects.push(title);
                    subjects.len() - 1
                });
                current = Some(index);
            }
            continue;
        }

        let targets = wiki_targets(line);
        // A bullet or a line carrying a link is graph-shaped; anything else is
        // prose, and enough prose means this note belongs to the LLM.
        if is_bullet(line) || !targets.is_empty() {
            graph_lines += 1;
        } else {
            prose += 1;
        }

        let Some(subject) = current else { continue };
        if targets.is_empty() {
            continue;
        }

        let relation = if targets.len() == 1 {
            line_relation(line).unwrap_or_else(|| MENTION_RELATION.to_string())
        } else {
            // Several links on one line: any trailing text describes the line,
            // not the individual relationships.
            MENTION_RELATION.to_string()
        };

        for target in targets {
            let source = &subjects[subject];
            if same_label(source, &target) {
                continue;
            }
            let key = format!(
                "{}|{}|{}",
                source.to_lowercase(),
                target.to_lowercase(),
                relation.to_lowercase()
            );
            if !edge_keys.insert(key) {
                continue;
            }

            ensure_node(&mut node_labels, &mut node_index, source);
            ensure_node(&mut node_labels, &mut node_index, &target);

            edges.push(GraphEdge {
                source_label: source.clone(),
                target_label: target,
                relation: relation.clone(),
                confidence: class_for(&relation).to_string(),
                evidence: Some(clamp_evidence(line)),
                ..GraphEdge::default()
            });
        }
    }

    let total = headings + graph_lines + prose;
    if total == 0 {
        return None;
    }
    let graph_shaped =
        (headings + graph_lines) * 100 >= total * STRUCTURED_RATIO_PERCENT;
    if !graph_shaped || edges.is_empty() {
        return None;
    }

    Some(GraphDelta {
        nodes: node_labels
            .into_iter()
            .map(|label| GraphNode {
                id: None,
                label,
                kind: DEFAULT_KIND.to_string(),
            })
            .collect(),
        edges,
    })
}

/// `# Title` → `Title`. Requires the space so `#tag` is not a heading, and
/// strips optional closing hashes.
/// `# Title` → level and title. Requires the space so `#tag` is not a
/// heading, and strips optional closing hashes.
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = line[level..].strip_prefix(' ')?;
    let title = rest.trim().trim_end_matches('#').trim();
    (!title.is_empty()).then(|| (level, title.to_string()))
}

/// The shallowest heading level used in the note (outside code fences).
fn subject_heading_level(text: &str) -> Option<usize> {
    let mut level: Option<usize> = None;
    let mut fenced = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some((found, _)) = heading(line) {
            level = Some(level.map_or(found, |shallowest| shallowest.min(found)));
        }
    }
    level
}

fn is_bullet(line: &str) -> bool {
    if ["- ", "* ", "+ "].iter().any(|m| line.starts_with(m)) {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && matches!(line[digits..].chars().next(), Some('.') | Some(')'))
}

/// Labels of every `[[wikilink]]` on the line: aliases (`|`) and heading
/// anchors (`#`) are dropped.
fn wiki_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = after[..end].split('|').next().unwrap_or("");
        let target = inner.split('#').next().unwrap_or("").trim();
        if !target.is_empty() && target.chars().count() <= 120 {
            targets.push(target.to_string());
        }
        rest = &after[end + 2..];
    }

    targets
}

/// The relation stated after a single link: `- [[Hogwarts]] — studies at`.
fn line_relation(line: &str) -> Option<String> {
    let open = line.find("[[")?;
    let after = &line[open + 2..];
    let close = after.find("]]")?;
    let tail = after[close + 2..].trim_start();

    let tail = ["—", "–", "→", "::", ":", "->", "=>", "- "]
        .iter()
        .find_map(|sep| tail.strip_prefix(sep))?;

    let relation = tail
        .trim()
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '*' | '`'));
    (!relation.is_empty()).then(|| relation.to_string())
}

fn clamp_evidence(line: &str) -> String {
    if line.chars().count() <= MAX_EVIDENCE_CHARS {
        return line.to_string();
    }
    line.chars().take(MAX_EVIDENCE_CHARS - 1).collect::<String>() + "…"
}

fn same_label(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn class_for(relation: &str) -> &'static str {
    if is_generic_relation(relation) {
        confidence::INFERRED
    } else {
        confidence::EXTRACTED
    }
}

fn ensure_node(
    labels: &mut Vec<String>,
    index: &mut HashMap<String, usize>,
    label: &str,
) -> usize {
    *index
        .entry(label.to_lowercase())
        .or_insert_with(|| {
            labels.push(label.to_string());
            labels.len() - 1
        })
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn from_json(text: &str) -> Option<GraphDelta> {
    let value: Value = serde_json::from_str(text).ok()?;
    let mut declared: Vec<(String, String)> = Vec::new();
    let mut proposed: Vec<(String, String, String)> = Vec::new();

    match &value {
        Value::Object(object) => {
            if let Some(nodes) = object.get("nodes").and_then(Value::as_array) {
                for item in nodes {
                    add_declared_node(&mut declared, item);
                }
            }
            if let Some(edges) = object.get("edges").and_then(Value::as_array) {
                for item in edges {
                    if let Some(edge) = read_edge(item) {
                        proposed.push(edge);
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                add_declared_node(&mut declared, item);
                // Nested form: { "label": "Ada", "relations": [{ "target", "relation" }] }
                let Some(object) = item.as_object() else {
                    continue;
                };
                let Some(source) = str_field(object, &["label", "name"]) else {
                    continue;
                };
                let Some(relations) = object.get("relations").and_then(Value::as_array) else {
                    continue;
                };
                for relation in relations {
                    let Some(object) = relation.as_object() else { continue };
                    let Some(target) = str_field(object, &["target", "to", "target_label"]) else {
                        continue;
                    };
                    let relation = str_field(object, &["relation", "rel", "type"])
                        .unwrap_or(MENTION_RELATION);
                    proposed.push((source.to_string(), target.to_string(), relation.to_string()));
                }
            }
        }
        _ => return None,
    }

    // Endpoints named only in an edge are still concepts the note states.
    for (source, target, _) in &proposed {
        if !declared.iter().any(|(label, _)| same_label(label, source)) {
            declared.push((source.clone(), DEFAULT_KIND.to_string()));
        }
        if !declared.iter().any(|(label, _)| same_label(label, target)) {
            declared.push((target.clone(), DEFAULT_KIND.to_string()));
        }
    }

    if declared.is_empty() {
        return None;
    }

    let mut edges = Vec::new();
    let mut seen = HashSet::new();
    for (source, target, relation) in proposed {
        if same_label(&source, &target) {
            continue;
        }
        let key = format!(
            "{}|{}|{}",
            source.to_lowercase(),
            target.to_lowercase(),
            relation.to_lowercase()
        );
        if !seen.insert(key) {
            continue;
        }
        edges.push(GraphEdge {
            source_label: source,
            target_label: target,
            confidence: class_for(&relation).to_string(),
            relation,
            ..GraphEdge::default()
        });
    }

    Some(GraphDelta {
        nodes: declared
            .into_iter()
            .map(|(label, kind)| GraphNode {
                id: None,
                label,
                kind,
            })
            .collect(),
        edges,
    })
}

fn add_declared_node(nodes: &mut Vec<(String, String)>, item: &Value) {
    let Some(object) = item.as_object() else { return };
    let Some(label) = str_field(object, &["label", "name"]) else {
        return;
    };
    if nodes.iter().any(|(existing, _)| same_label(existing, label)) {
        return;
    }
    let kind = str_field(object, &["kind", "type"]).unwrap_or(DEFAULT_KIND);
    nodes.push((label.to_string(), kind.to_string()));
}

fn read_edge(item: &Value) -> Option<(String, String, String)> {
    let object = item.as_object()?;
    let source = str_field(object, &["source", "from", "source_label"])?;
    let target = str_field(object, &["target", "to", "target_label"])?;
    let relation = str_field(object, &["relation", "rel", "type"]).unwrap_or(MENTION_RELATION);
    Some((source.to_string(), target.to_string(), relation.to_string()))
}

fn str_field<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(delta: &GraphDelta) -> Vec<&str> {
        delta.nodes.iter().map(|n| n.label.as_str()).collect()
    }

    fn relations(delta: &GraphDelta) -> Vec<(&str, &str, &str)> {
        delta
            .edges
            .iter()
            .map(|e| {
                (
                    e.source_label.as_str(),
                    e.relation.as_str(),
                    e.target_label.as_str(),
                )
            })
            .collect()
    }

    const OUTLINE: &str = "# Harry Potter
A wizard.

## Relationships
- [[Hogwarts]] — studies at
- [[Ron Weasley]] — friend of
";

    #[test]
    fn parses_heading_owned_links() {
        let out = extract(OUTLINE).expect("outline should parse");
        assert_eq!(out.format, FORMAT_MARKDOWN);
        assert_eq!(
            labels(&out.delta),
            vec!["Harry Potter", "Hogwarts", "Ron Weasley"]
        );
        assert_eq!(
            relations(&out.delta),
            vec![
                ("Harry Potter", "studies at", "Hogwarts"),
                ("Harry Potter", "friend of", "Ron Weasley"),
            ]
        );
    }

    #[test]
    fn marks_stated_edges_extracted_with_the_source_line() {
        let out = extract(OUTLINE).unwrap();
        for edge in &out.delta.edges {
            assert_eq!(edge.confidence, confidence::EXTRACTED);
        }
        assert_eq!(
            out.delta.edges[0].evidence.as_deref(),
            Some("- [[Hogwarts]] — studies at")
        );
    }

    #[test]
    fn generic_relation_is_inferred_not_extracted() {
        let out = extract("# Mercury\n- [[Venus]] — related to\n").unwrap();
        assert_eq!(out.delta.edges[0].confidence, confidence::INFERRED);
    }

    #[test]
    fn link_without_relation_becomes_a_mention() {
        let out = extract("# Harry\n- [[Hogwarts]]\n").unwrap();
        assert_eq!(
            relations(&out.delta),
            vec![("Harry", "links to", "Hogwarts")]
        );
    }

    #[test]
    fn prose_reference_under_a_heading_is_a_mention() {
        let out = extract("# Harry\nHarry finally returns to [[Hogwarts]].\n").unwrap();
        assert_eq!(
            relations(&out.delta),
            vec![("Harry", "links to", "Hogwarts")]
        );
    }

    #[test]
    fn each_heading_owns_the_links_beneath_it() {
        let note = "# Ada Lovelace
- [[Analytical Engine]] — programmed

# Charles Babbage
- [[Analytical Engine]] — designed
";
        let out = extract(note).unwrap();
        assert_eq!(
            relations(&out.delta),
            vec![
                ("Ada Lovelace", "programmed", "Analytical Engine"),
                ("Charles Babbage", "designed", "Analytical Engine"),
            ]
        );
    }

    #[test]
    fn resolves_aliases_and_heading_anchors() {
        let out = extract("# Ada\n- [[Analytical Engine|the engine]] — designed\n").unwrap();
        assert_eq!(out.delta.edges[0].target_label, "Analytical Engine");

        let out = extract("# Ada\n- [[Engines#Difference]] — built\n").unwrap();
        assert_eq!(out.delta.edges[0].target_label, "Engines");
    }

    #[test]
    fn skips_links_inside_code_fences() {
        let note = "# Ada
```md
- [[Example]] — not a real edge
```
- [[Analytical Engine]] — programmed
";
        let out = extract(note).unwrap();
        assert_eq!(
            relations(&out.delta),
            vec![("Ada", "programmed", "Analytical Engine")]
        );
    }

    #[test]
    fn skips_self_links_and_duplicates() {
        let note = "# Hogwarts
- [[Hogwarts]] — is
- [[Gryffindor]] — houses
- [[Gryffindor]] — houses
";
        let out = extract(note).unwrap();
        assert_eq!(
            relations(&out.delta),
            vec![("Hogwarts", "houses", "Gryffindor")]
        );
    }

    #[test]
    fn ignores_notes_without_a_heading() {
        assert!(extract("- [[Hogwarts]] — studies at\n").is_none());
    }

    #[test]
    fn ignores_headings_without_links() {
        assert!(extract("# Harry Potter\n\n## Relationships\n").is_none());
    }

    #[test]
    fn prose_notes_are_left_to_the_llm() {
        let note = "# Notes
Harry is a wizard who studies at Hogwarts and is friends with Ron.
Hermione is the best student in their year, and she keeps them alive.
- [[Hogwarts]] — maybe
";
        assert!(extract(note).is_none(), "mostly prose must not be hijacked");
    }

    #[test]
    fn parses_canonical_json() {
        let note = r#"{
            "nodes": [
                { "label": "Ada Lovelace", "kind": "person" },
                { "label": "Analytical Engine", "kind": "machine" }
            ],
            "edges": [
                { "source": "Ada Lovelace", "target": "Analytical Engine", "relation": "programmed" }
            ]
        }"#;
        let out = extract(note).expect("json should parse");
        assert_eq!(out.format, FORMAT_JSON);
        assert_eq!(out.delta.nodes.len(), 2);
        assert_eq!(out.delta.nodes[0].kind, "person");
        assert_eq!(
            relations(&out.delta),
            vec![("Ada Lovelace", "programmed", "Analytical Engine")]
        );
        assert_eq!(out.delta.edges[0].confidence, confidence::EXTRACTED);
    }

    #[test]
    fn parses_json_node_array_with_nested_relations() {
        let note = r#"[
            { "name": "Ada", "kind": "person",
              "relations": [{ "target": "Engine", "relation": "programmed" }] }
        ]"#;
        let out = extract(note).unwrap();
        assert_eq!(labels(&out.delta), vec!["Ada", "Engine"]);
        assert_eq!(relations(&out.delta), vec![("Ada", "programmed", "Engine")]);
    }

    #[test]
    fn json_edge_endpoints_become_nodes() {
        let note = r#"{"edges": [{"from": "A", "to": "B", "rel": "knows"}]}"#;
        let out = extract(note).unwrap();
        assert_eq!(labels(&out.delta), vec!["A", "B"]);
    }

    #[test]
    fn json_without_a_nodes_key_is_left_to_the_llm() {
        let note = r#"{"users": [{"name": "Ada"}], "total": 1}"#;
        assert!(extract(note).is_none());
    }

    #[test]
    fn generic_json_relation_is_inferred() {
        let note = r#"{"nodes":[{"label":"A"},{"label":"B"}],"edges":[{"source":"A","target":"B","relation":"associated with"}]}"#;
        let out = extract(note).unwrap();
        assert_eq!(out.delta.edges[0].confidence, confidence::INFERRED);
    }

    #[test]
    fn empty_and_plain_text_notes_are_not_structured() {
        assert!(extract("").is_none());
        assert!(extract("   \n\n").is_none());
        assert!(extract("Harry studies at Hogwarts.").is_none());
    }
}
