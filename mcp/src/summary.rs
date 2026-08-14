//! One-shot LLM note summarization (best-effort, never blocks ingestion).

use std::sync::Arc;

use weave_core::llm::OpenCodeClient;

const SUMMARY_PROMPT: &str = r#"You are a personal memory assistant. Summarize the
user's note into AT MOST two short sentences, keeping concrete facts, proper
nouns, and relationships. Do not add anything not in the note.
Respond with strict JSON only: {"summary":"..."}"#;

/// Summarize a note into ≤2 sentences. Returns `None` when the LLM is
/// unavailable or the call fails.
pub async fn summarize(llm: &OpenCodeClient, content: &str) -> Option<String> {
    if !llm.available() || content.trim().is_empty() {
        return None;
    }
    match llm.chat_json(SUMMARY_PROMPT, content).await {
        Ok(out) => out.json["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        Err(e) => {
            tracing::warn!(error = %e, "summarize failed");
            None
        }
    }
}

/// Convenience for callers holding an `Arc`.
pub async fn summarize_arc(llm: &Arc<OpenCodeClient>, content: &str) -> Option<String> {
    summarize(llm.as_ref(), content).await
}
