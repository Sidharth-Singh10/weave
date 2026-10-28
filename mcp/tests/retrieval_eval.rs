//! Retrieval quality gate.
//!
//! Measures recall@k and MRR@k of the shared ranking formula over a fixed
//! corpus, so a change to weights or to the hub penalty cannot quietly make
//! retrieval worse. Runs with no database, no network, and no embedder: the
//! corpus supplies the features each candidate would have.
//!
//! This evaluates the *ranker*, not candidate generation (the SQL layers that
//! decide which entities are proposed at all). When a Postgres instance is
//! reachable, `tests/memory_test.rs` covers the end-to-end path separately.
//!
//! The corpus is `tests/fixtures/retrieval_eval.json`; add a case there
//! whenever retrieval is found to be wrong.

use serde::Deserialize;
use weave_mcp::retrieval::{score, Signals};

#[derive(Debug, Deserialize)]
struct Corpus {
    k: usize,
    thresholds: Thresholds,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Thresholds {
    recall_at_k: f64,
    mrr_at_k: f64,
    min_case_recall: f64,
}

#[derive(Debug, Deserialize)]
struct Case {
    query: String,
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    label: String,
    degree: usize,
    #[serde(default)]
    mentioned: bool,
    #[serde(default)]
    keyword: bool,
    #[serde(default)]
    neighbor: bool,
    similarity: Option<f32>,
    /// 0 irrelevant .. 3 the answer.
    relevance: u8,
}

impl Candidate {
    fn signals(&self) -> Signals {
        Signals {
            mentioned: self.mentioned,
            keyword: self.keyword,
            neighbor: self.neighbor,
            similarity: self.similarity,
            degree: self.degree,
        }
    }

    fn score(&self) -> f64 {
        score(&self.signals())
    }
}

fn corpus() -> Corpus {
    serde_json::from_str(include_str!("fixtures/retrieval_eval.json"))
        .expect("retrieval eval corpus must parse")
}

/// Rank a case's candidates exactly as `retrieve_entities` would: by score,
/// with the label as a stable tie-break.
fn rank(case: &Case) -> Vec<&Candidate> {
    let mut ranked: Vec<&Candidate> = case.candidates.iter().collect();
    ranked.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.label.cmp(&b.label))
    });
    ranked
}

fn recall_at_k(ranked: &[&Candidate], k: usize) -> f64 {
    let relevant = ranked.iter().filter(|c| c.relevance > 0).count();
    if relevant == 0 {
        return 1.0;
    }
    let hits = ranked.iter().take(k).filter(|c| c.relevance > 0).count();
    hits as f64 / relevant as f64
}

fn mrr_at_k(ranked: &[&Candidate], k: usize) -> f64 {
    ranked
        .iter()
        .take(k)
        .position(|c| c.relevance > 0)
        .map(|index| 1.0 / (index as f64 + 1.0))
        .unwrap_or(0.0)
}

#[test]
fn corpus_is_not_degenerate() {
    let corpus = corpus();
    assert!(corpus.k >= 2, "k must be at least 2");
    assert!(
        corpus.cases.len() >= 4,
        "a corpus of {} cases cannot detect regressions",
        corpus.cases.len()
    );
    for case in &corpus.cases {
        assert!(
            case.candidates.iter().any(|c| c.relevance > 0),
            "case {:?} has no relevant candidate, so its recall is meaningless",
            case.query
        );
        assert!(
            !case.query.trim().is_empty(),
            "every case needs a query"
        );
    }
}

#[test]
fn retrieval_quality_meets_baseline() {
    let corpus = corpus();
    let k = corpus.k;

    let mut recall_sum = 0.0;
    let mut mrr_sum = 0.0;

    for case in &corpus.cases {
        let ranked = rank(case);
        let recall = recall_at_k(&ranked, k);
        let mrr = mrr_at_k(&ranked, k);
        recall_sum += recall;
        mrr_sum += mrr;

        eprintln!(
            "recall@{k}={recall:.3} mrr@{k}={mrr:.3}  ranking=[{}]  query={:?}",
            ranked
                .iter()
                .map(|c| format!("{}:{:.3}(rel{})", c.label, c.score(), c.relevance))
                .collect::<Vec<_>>()
                .join(", "),
            case.query
        );

        assert!(
            recall >= corpus.thresholds.min_case_recall,
            "case {:?} fell to recall@{k}={recall:.3}; expected at least {:.2}",
            case.query,
            corpus.thresholds.min_case_recall
        );
    }

    let cases = corpus.cases.len() as f64;
    let recall = recall_sum / cases;
    let mrr = mrr_sum / cases;

    eprintln!("aggregate recall@{k}={recall:.3} mrr@{k}={mrr:.3} over {cases} cases");

    assert!(
        recall >= corpus.thresholds.recall_at_k,
        "recall@{k} regressed to {recall:.3}, below the {:.2} baseline",
        corpus.thresholds.recall_at_k
    );
    assert!(
        mrr >= corpus.thresholds.mrr_at_k,
        "mrr@{k} regressed to {mrr:.3}, below the {:.2} baseline",
        corpus.thresholds.mrr_at_k
    );
}
