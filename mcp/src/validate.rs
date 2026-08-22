//! Deterministic claim validation (V2).
//!
//! "LLM proposes, system verifies": extraction is free-form; this module
//! decides what is allowed to become memory. Pure functions only — no DB, no
//! LLM — so they are trivially testable.

/// Labels that carry no information; never allowed as entities.
pub const PLACEHOLDER_LABELS: &[&str] = &[
    "none", "n/a", "na", "unknown", "thing", "something", "stuff", "etc", "etc.", "it", "this",
    "that", "them", "someone", "somebody", "anyone", "some", "any", "one", "new", "old", "note",
];

/// Words that flip a claim into `negated`.
const NEGATION_WORDS: &[&str] = &[
    "not", "no", "never", "without", "isn't", "isnt", "doesn't", "doesnt", "don't", "dont",
    "cannot", "can't", "cant", "neither", "nor", "won't", "wont", "no longer",
];

/// Hedge words that lower certainty to `suggested`.
const HEDGE_WORDS: &[&str] = &[
    "may", "might", "suggests", "suggest", "possibly", "perhaps", "could", "would", "appears",
    "likely", "unlikely", "correlates", "correlated", "associated with", "associates", "implies",
    "seems", "tends to", "reportedly",
];

/// Conditional markers -> `conditional`.
const CONDITIONAL_WORDS: &[&str] = &["if", "when", "whenever", "provided that", "assuming"];

/// A validated edge candidate. The raw LLM edge may be rejected entirely.
#[derive(Debug, Clone)]
pub struct ClaimCandidate {
    pub subject_label: String,
    pub predicate: String,
    pub object_label: String,
    pub modality: String,
    pub confidence: f32,
    pub evidence_span: Option<String>,
    pub evidence_offset: Option<i32>,
    /// True when the note text does not reference the claim's endpoint labels.
    pub supported: bool,
}

/// Validate an entity label. Returns a rejection reason when invalid.
pub fn validate_label(label: &str) -> Result<(), String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err("empty label".into());
    }
    if trimmed.chars().count() > 80 {
        return Err("label too long (>80 chars)".into());
    }
    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return Err("label has no alphanumeric characters".into());
    }
    if PLACEHOLDER_LABELS
        .iter()
        .any(|p| p.eq_ignore_ascii_case(trimmed))
    {
        return Err("placeholder label".into());
    }
    Ok(())
}

/// Normalize + validate a predicate. Returns the canonical predicate or a
/// rejection reason.
pub fn normalize_predicate(predicate: &str) -> Result<String, String> {
    let normalized: String = predicate
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() {
        return Err("empty predicate".into());
    }
    if normalized.chars().count() > 60 {
        return Err("predicate too long (>60 chars)".into());
    }
    if !normalized.chars().any(|c| c.is_alphabetic()) {
        return Err("predicate has no letters".into());
    }
    Ok(normalized)
}

/// Does `haystack` contain any of `needles` as a case-insensitive substring?
fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let hay = haystack.to_lowercase();
    needles.iter().any(|n| hay.contains(&n.to_lowercase()))
}

/// Infer the modality of a claim from the note text around the evidence.
pub fn infer_modality(evidence: &str) -> &'static str {
    if contains_any(evidence, NEGATION_WORDS) {
        "negated"
    } else if contains_any(evidence, CONDITIONAL_WORDS) {
        "conditional"
    } else if contains_any(evidence, HEDGE_WORDS) {
        "suggested"
    } else {
        "asserted"
    }
}

/// Confidence for a modality (deterministic).
pub fn confidence_for_modality(modality: &str) -> f32 {
    match modality {
        "negated" => 0.8,
        "suggested" => 0.5,
        "conditional" => 0.4,
        _ => 1.0,
    }
}

/// Position of `label` in the lowercased note: exact label first, then the
/// first word when it is a meaningful name fragment ("Harry Potter" matches
/// "Harry"). Short/common first words are skipped to avoid false hits.
fn label_position(lower_note: &str, label: &str) -> Option<usize> {
    let l = label.to_lowercase();
    if let Some(pos) = lower_note.find(&l) {
        return Some(pos);
    }
    if let Some(first) = l.split_whitespace().next() {
        if first.chars().count() >= 4 {
            if let Some(pos) = lower_note.find(first) {
                return Some(pos);
            }
        }
    }
    None
}

/// Extract the supporting sentence(s) covering the first occurrence of both
/// `subject` and `object` in the note (case-insensitive). Returns
/// (span, offset). `None` when either endpoint label is absent — an
/// unsupported claim (the note must name both entities).
pub fn find_evidence_span(
    note: &str,
    subject_label: &str,
    object_label: &str,
) -> Option<(String, i32)> {
    let lower = note.to_lowercase();
    let (subject_pos, object_pos) = (
        label_position(&lower, subject_label)?,
        label_position(&lower, object_label)?,
    );
    let start = subject_pos.min(object_pos);
    let end_pos = subject_pos.max(object_pos);

    // Sentence boundaries covering both mentions; cap the window at 400 chars.
    let before = note[..start]
        .rfind(['.', '!', '?'])
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut end = note[end_pos..]
        .find(['.', '!', '?'])
        .map(|i| end_pos + i + 1)
        .unwrap_or(note.len());
    if end - before > 400 {
        end = before + 400;
    }
    let span = note[before..end].trim().to_string();
    Some((span, before as i32))
}

/// Build a validated claim candidate from a raw LLM edge, or a rejection
/// reason. Self-loops are malformed.
pub fn validate_claim(
    note: &str,
    subject_label: &str,
    predicate: &str,
    object_label: &str,
) -> Result<ClaimCandidate, String> {
    validate_label(subject_label)?;
    validate_label(object_label)?;
    if subject_label.eq_ignore_ascii_case(object_label) {
        return Err("self-loop claim".into());
    }
    let predicate = normalize_predicate(predicate)?;

    let (evidence_span, evidence_offset) =
        match find_evidence_span(note, subject_label, object_label) {
            Some((span, offset)) => (Some(span), Some(offset)),
            None => (None, None),
        };
    let evidence = evidence_span.as_deref().unwrap_or(note);
    let modality = infer_modality(evidence);
    let confidence = confidence_for_modality(&modality);
    let supported = evidence_span.is_some();

    Ok(ClaimCandidate {
        subject_label: subject_label.trim().to_string(),
        predicate,
        object_label: object_label.trim().to_string(),
        modality: modality.to_string(),
        confidence,
        evidence_span,
        evidence_offset,
        supported,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_validation() {
        assert_eq!(validate_label("Harry Potter"), Ok(()));
        assert_eq!(validate_label("  "), Err("empty label".into()));
        assert_eq!(validate_label("unknown"), Err("placeholder label".into()));
        assert_eq!(validate_label("N/A"), Err("placeholder label".into()));
        assert_eq!(validate_label("!!!"), Err("label has no alphanumeric characters".into()));
        assert_eq!(validate_label(&"x".repeat(90)), Err("label too long (>80 chars)".into()));
    }

    #[test]
    fn predicate_normalization() {
        assert_eq!(normalize_predicate("  Friend   Of "), Ok("friend of".into()));
        assert_eq!(normalize_predicate(""), Err("empty predicate".into()));
        assert_eq!(normalize_predicate("12345"), Err("predicate has no letters".into()));
        assert_eq!(normalize_predicate("  "), Err("empty predicate".into()));
    }

    #[test]
    fn modality_inference() {
        assert_eq!(infer_modality("Harry is not afraid of spiders."), "negated");
        assert_eq!(infer_modality("Harry never visits the library."), "negated");
        assert_eq!(infer_modality("Coffee may improve focus."), "suggested");
        assert_eq!(infer_modality("Study suggests a link."), "suggested");
        assert_eq!(infer_modality("If it rains, the match is off."), "conditional");
        assert_eq!(infer_modality("Harry studies at Hogwarts."), "asserted");
    }

    #[test]
    fn evidence_span_captures_sentence() {
        let note = "Harry visited the library yesterday. He studies at Hogwarts. Ron left.";
        let (span, offset) = find_evidence_span(note, "Harry", "Hogwarts").unwrap();
        assert!(span.contains("Harry"));
        assert!(span.contains("Hogwarts"));
        assert!(span.len() <= 400);
        assert_eq!(offset, 0);
    }

    #[test]
    fn evidence_span_none_when_unrelated() {
        assert!(find_evidence_span("Completely unrelated text.", "Hogwarts", "Quidditch").is_none());
    }

    #[test]
    fn validates_and_detects_unsupported() {
        let ok = validate_claim("Harry studies at Hogwarts.", "Harry Potter", "studies at", "Hogwarts").unwrap();
        assert!(ok.supported);
        assert_eq!(ok.modality, "asserted");

        let unsupported = validate_claim("Some unrelated note.", "Hogwarts", "contains", "The Room of Requirement").unwrap();
        assert!(!unsupported.supported);

        assert!(
            matches!(
                validate_claim("Harry studies at Hogwarts.", "Harry", "lives in", "Harry"),
                Err(reason) if reason == "self-loop claim"
            ),
            "self-loops must be rejected"
        );
    }

    #[test]
    fn negation_claim_flags_negated() {
        let c = validate_claim(
            "Ron does not like spiders.",
            "Ron",
            "likes",
            "spiders",
        )
        .unwrap();
        assert_eq!(c.modality, "negated");
        assert_eq!(c.confidence, 0.8);
        assert!(c.supported);
    }
}