//! Selective deep verification (V4).
//!
//! Most writes are validated deterministically (V2) and never reach the LLM
//! again. High-risk claims — ambiguous entity resolution, contradiction with
//! an existing high-confidence claim, uncertain/comparative language, or
//! unsupported by evidence — are reviewed by a narrow, structured verifier
//! call. The verifier is an exception path with hard caps; normal notes pay
//! nothing.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use weave_core::llm::OpenCodeClient;

/// Hard cap on verifier calls per note — the risk policy must never let
/// verification cost grow unboundedly.
pub const VERIFIER_MAX_CALLS_PER_NOTE: usize = 5;

/// Predicates that signal comparative/scientific claims worth verifying.
const COMPARATIVE_MARKERS: &[&str] = &[
    "better than", "worse than", "causes", "cause", "increases", "increased", "decreases",
    "decreased", "prevents", "correlates with", "associated with", "improves", "improve",
    "reduces", "reduced", "enhances", "raises", "lowers", "accelerates", "slows down", "leads to",
];

const VERIFIER_SYSTEM_PROMPT: &str = r#"You are a memory verifier. You review ONE candidate
memory claim against the evidence provided and return a strict JSON decision.

Rules:
- Only accept a claim that the provided evidence actually supports.
- A claim is unsupported if the source text does not name both entities or does not
  support the predicate.
- A "scoped_negation" is when a negation word applies only to part of the sentence
  (e.g. "X provides Y without Z" asserts X provides Y; only Z is negated). Correct
  the modality to the asserted value.
- Ambiguous entity resolution: when candidate existing entities are listed, choose
  the canonical one (ids are provided) only if clearly the same concept; otherwise
  leave the id null.
- Never invent evidence. Prefer "quarantine" over a confident wrong answer.
Respond with strict JSON only, matching:
{"decision":"accept|reject|quarantine","confidence":0..1,
 "reason_code":"supported_by_evidence|unsupported|contradictory_evidence|ambiguous_entity|scoped_negation|uncertain|malformed",
 "canonical_subject_id":"uuid|null","canonical_object_id":"uuid|null",
 "corrected_modality":"asserted|negated|suggested|conditional|null",
 "explanation":"one short sentence"}"#;

/// Why a claim was flagged for verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskReason {
    Unsupported,
    UncertainModality,
    ComparativeLanguage,
    AmbiguousEntity,
    Contradiction,
}

impl RiskReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskReason::Unsupported => "unsupported",
            RiskReason::UncertainModality => "uncertain_modality",
            RiskReason::ComparativeLanguage => "comparative_language",
            RiskReason::AmbiguousEntity => "ambiguous_entity",
            RiskReason::Contradiction => "contradiction",
        }
    }
}

/// The structured verifier decision (strict JSON contract).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierDecision {
    pub decision: String,
    pub confidence: f32,
    pub reason_code: String,
    #[serde(default)]
    pub canonical_subject_id: Option<String>,
    #[serde(default)]
    pub canonical_object_id: Option<String>,
    #[serde(default)]
    pub corrected_modality: Option<String>,
    #[serde(default)]
    pub explanation: String,
}

/// Everything the verifier needs: the claim, its evidence, and any conflict /
/// ambiguity context.
#[derive(Debug, Clone)]
pub struct EvidenceBundle {
    pub subject_label: String,
    pub predicate: String,
    pub object_label: String,
    pub modality: String,
    pub confidence: f32,
    pub evidence_span: Option<String>,
    pub note_content: String,
    /// Conflicting claims as "subj -[pred]-> obj (modality, status)".
    pub conflicting_claims: Vec<String>,
    /// Candidate labels for an ambiguous subject resolution.
    pub ambiguous_subject_candidates: Vec<String>,
    /// Candidate labels for an ambiguous object resolution.
    pub ambiguous_object_candidates: Vec<String>,
    /// Top retrieval anchors (graph context).
    pub anchors: Vec<String>,
}

/// Deterministic risk assessment. Returns empty when the claim is safe to
/// commit without a verifier call.
pub fn assess_risk(
    supported: bool,
    modality: &str,
    predicate: &str,
    ambiguous_subject: bool,
    ambiguous_object: bool,
    has_conflict: bool,
) -> Vec<RiskReason> {
    let mut reasons = Vec::new();
    if !supported {
        reasons.push(RiskReason::Unsupported);
    }
    if modality == "suggested" || modality == "conditional" {
        reasons.push(RiskReason::UncertainModality);
    }
    let lower = predicate.to_lowercase();
    if COMPARATIVE_MARKERS.iter().any(|m| lower.contains(m)) {
        reasons.push(RiskReason::ComparativeLanguage);
    }
    if ambiguous_subject || ambiguous_object {
        reasons.push(RiskReason::AmbiguousEntity);
    }
    if has_conflict {
        reasons.push(RiskReason::Contradiction);
    }
    reasons
}

/// Build the verifier's user message from the evidence bundle.
pub fn build_verifier_user_prompt(bundle: &EvidenceBundle) -> String {
    let evidence = bundle.evidence_span.clone().unwrap_or_default();
    let conflicts = if bundle.conflicting_claims.is_empty() {
        "(none)".to_string()
    } else {
        bundle.conflicting_claims.join("; ")
    };
    let subj_cands = if bundle.ambiguous_subject_candidates.is_empty() {
        "(none)".to_string()
    } else {
        bundle.ambiguous_subject_candidates.join(", ")
    };
    let obj_cands = if bundle.ambiguous_object_candidates.is_empty() {
        "(none)".to_string()
    } else {
        bundle.ambiguous_object_candidates.join(", ")
    };
    let anchors = if bundle.anchors.is_empty() {
        "(none)".to_string()
    } else {
        bundle.anchors.join(", ")
    };
    format!(
        "CANDIDATE CLAIM\n--------------\n{} -[{}]-> {} (modality: {}, confidence: {:.2})\n\n\
         EVIDENCE\n--------\n{}\n\nFULL SOURCE TEXT\n----------------\n{}\n\n\
         CONFLICTING CLAIMS\n-------------------\n{}\n\n\
         AMBIGUOUS SUBJECT CANDIDATES\n----------------------------\n{}\n\n\
         AMBIGUOUS OBJECT CANDIDATES\n---------------------------\n{}\n\n\
         RELEVANT GRAPH CONTEXT\n-----------------------\n{}",
        bundle.subject_label,
        bundle.predicate,
        bundle.object_label,
        bundle.modality,
        bundle.confidence,
        evidence,
        bundle.note_content,
        conflicts,
        subj_cands,
        obj_cands,
        anchors,
    )
}

fn valid_decision(d: &str) -> bool {
    matches!(d, "accept" | "reject" | "quarantine")
}

fn valid_modality(m: &str) -> bool {
    matches!(m, "asserted" | "negated" | "suggested" | "conditional")
}

/// Clamp and sanitize a raw verifier output so downstream code can trust it.
pub fn normalize_decision(mut d: VerifierDecision) -> VerifierDecision {
    if !valid_decision(&d.decision) {
        d.decision = "quarantine".to_string();
    }
    d.confidence = d.confidence.clamp(0.0, 1.0);
    if let Some(m) = &d.corrected_modality {
        if !valid_modality(m) {
            d.corrected_modality = None;
        }
    }
    d
}

/// Run the verifier. Returns the parsed decision, normalized.
pub async fn verify_claim(
    llm: &Arc<OpenCodeClient>,
    bundle: &EvidenceBundle,
) -> anyhow::Result<VerifierDecision> {
    if !llm.available() {
        anyhow::bail!("llm unavailable");
    }
    let user = build_verifier_user_prompt(bundle);
    let out = llm.chat_json(VERIFIER_SYSTEM_PROMPT, &user).await?;
    let d: VerifierDecision = serde_json::from_value(out.json)?;
    Ok(normalize_decision(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_triggers() {
        // Safe claim: supported, asserted, no markers, no ambiguity/conflict.
        assert!(assess_risk(true, "asserted", "studies at", false, false, false).is_empty());
        // Unsupported fires.
        assert_eq!(
            assess_risk(false, "asserted", "studies at", false, false, false),
            vec![RiskReason::Unsupported]
        );
        // Uncertain modality fires.
        assert!(assess_risk(true, "suggested", "studies at", false, false, false)
            .contains(&RiskReason::UncertainModality));
        // Comparative predicate fires.
        assert!(assess_risk(true, "asserted", "increases", false, false, false)
            .contains(&RiskReason::ComparativeLanguage));
        // Ambiguity fires.
        assert!(assess_risk(true, "asserted", "studies at", true, false, false)
            .contains(&RiskReason::AmbiguousEntity));
        // Contradiction fires.
        assert!(assess_risk(true, "asserted", "studies at", false, false, true)
            .contains(&RiskReason::Contradiction));
    }

    #[test]
    fn bundle_prompt_contains_parts() {
        let bundle = EvidenceBundle {
            subject_label: "X".into(),
            predicate: "increases".into(),
            object_label: "Y".into(),
            modality: "suggested".into(),
            confidence: 0.5,
            evidence_span: Some("X increases Y in the study.".into()),
            note_content: "Full note text.".into(),
            conflicting_claims: vec!["X -[increases]-> Y (negated, active)".into()],
            ambiguous_subject_candidates: vec!["X Corp".into()],
            ambiguous_object_candidates: vec![],
            anchors: vec!["X".into()],
        };
        let prompt = build_verifier_user_prompt(&bundle);
        assert!(prompt.contains("X -[increases]-> Y (modality: suggested"));
        assert!(prompt.contains("X -[increases]-> Y (negated, active)"));
        assert!(prompt.contains("X Corp"));
        assert!(prompt.contains("Full note text."));
    }

    #[test]
    fn verdict_normalization() {
        let d = normalize_decision(VerifierDecision {
            decision: "banana".into(),
            confidence: 2.0,
            reason_code: "whatever".into(),
            canonical_subject_id: None,
            canonical_object_id: None,
            corrected_modality: Some("banana".into()),
            explanation: String::new(),
        });
        assert_eq!(d.decision, "quarantine");
        assert_eq!(d.confidence, 1.0);
        assert_eq!(d.corrected_modality, None);

        let ok = normalize_decision(VerifierDecision {
            decision: "accept".into(),
            confidence: -0.5,
            reason_code: "supported_by_evidence".into(),
            canonical_subject_id: None,
            canonical_object_id: None,
            corrected_modality: Some("asserted".into()),
            explanation: String::new(),
        });
        assert_eq!(ok.decision, "accept");
        assert_eq!(ok.confidence, 0.0);
        assert_eq!(ok.corrected_modality.as_deref(), Some("asserted"));
    }
}