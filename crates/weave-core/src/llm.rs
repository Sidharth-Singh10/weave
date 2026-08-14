use std::time::Duration;

use anyhow::{anyhow, Context};

/// Provider-reported token usage for one LLM call.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

/// A chat completion result: the parsed JSON plus any provider-reported usage.
#[derive(Debug)]
pub struct LlmOutput {
    pub json: serde_json::Value,
    pub usage: Option<TokenUsage>,
}

fn parse_usage(raw: &serde_json::Value) -> Option<TokenUsage> {
    let usage = raw.get("usage")?;
    if !usage.is_object() {
        return None;
    }
    let input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    let output_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    let total_tokens = usage.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    Some(TokenUsage { input_tokens, output_tokens, total_tokens })
}

/// OpenAI-compatible client for the opencode-go provider.
///
/// Fully standalone: configuration comes from environment variables only
/// (`OPENCODE_BASE_URL`, `OPENCODE_MODEL`, `OPENCODE_API_KEY`). When no API
/// key is present, [`OpenCodeClient::available`] returns false and callers
/// fall back to the deterministic mock extractor.
#[derive(Clone)]
pub struct OpenCodeClient {
    pub base_url: String,
    pub model: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl OpenCodeClient {
    pub fn from_env() -> Self {
        let base_url = std::env::var("OPENCODE_BASE_URL")
            .unwrap_or_else(|_| "https://opencode.ai/zen/go/v1".to_string());
        let model =
            std::env::var("OPENCODE_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
        let api_key = std::env::var("OPENCODE_API_KEY").ok();

        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build http client");

        Self {
            base_url,
            model,
            api_key,
            http,
        }
    }

    pub fn available(&self) -> bool {
        self.api_key.is_some()
    }

    /// Send a chat completion and return the parsed JSON the model produced,
    /// along with provider-reported token usage (if any).
    ///
    /// The model may wrap its JSON in markdown code fences; those are
    /// stripped before parsing. On a transient 5xx / 429 the request is
    /// retried once.
    pub async fn chat_json(&self, system: &str, user: &str) -> anyhow::Result<LlmOutput> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("no OPENCODE_API_KEY configured"))?;

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0.2,
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let build = || async {
            self.http
                .post(&url)
                .bearer_auth(key)
                .json(&body)
                .send()
                .await
                .map_err(|e| anyhow!(e))
        };

        let mut resp = build().await?;
        if !resp.status().is_success() && is_retryable(resp.status()) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            resp = build().await?;
        }

        let status = resp.status();
        let text = resp
            .text()
            .await
            .with_context(|| format!("read response body (status {status})"))?;

        if !status.is_success() {
            return Err(anyhow!("LLM request failed ({status}): {}", truncate(&text, 300)));
        }

        let raw: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| "LLM response was not valid JSON".to_string())?;

        let content = raw["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("LLM response missing message content"))?;

        let cleaned = strip_code_fences(content);
        let json = serde_json::from_str(cleaned)
            .with_context(|| format!("model output was not JSON: {}", truncate(cleaned, 200)))?;

        Ok(LlmOutput {
            json,
            usage: parse_usage(&raw),
        })
    }

    /// List model ids exposed by the endpoint (for the status endpoint).
    pub async fn list_models(&self) -> Vec<String> {
        let Some(key) = self.api_key.as_deref() else {
            return vec![];
        };
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self.http.get(&url).bearer_auth(key).send().await;
        let Ok(resp) = resp else {
            return vec![];
        };
        let Ok(resp) = resp.error_for_status() else {
            return vec![];
        };
        let Ok(json) = resp.json::<serde_json::Value>().await else {
            return vec![];
        };
        json["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn is_retryable(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || status.as_u16() == 429
}

fn strip_code_fences(content: &str) -> &str {
    let trimmed = content.trim();
    if let Some(stripped) = trimmed.strip_prefix("```") {
        let without_lang = match stripped.find('\n') {
            Some(idx) => &stripped[idx..],
            None => stripped,
        };
        let without_close = without_lang.strip_suffix("```").unwrap_or(without_lang);
        return without_close.trim();
    }
    trimmed
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_code_fences() {
        assert_eq!(
            strip_code_fences("```json\n{\"a\": 1}\n```"),
            "{\"a\": 1}"
        );
        assert_eq!(
            strip_code_fences("```\n{\"a\": 1}\n```"),
            "{\"a\": 1}"
        );
        assert_eq!(strip_code_fences("{\"a\": 1}"), "{\"a\": 1}");
    }

    #[test]
    fn from_env_resolves_defaults() {
        // Sandbox env so the test is hermetic (standalone, no host config).
        let mut env = std::env::vars_os()
            .filter(|(k, _)| {
                k != "OPENCODE_BASE_URL" && k != "OPENCODE_MODEL" && k != "OPENCODE_API_KEY"
            })
            .collect::<Vec<_>>();
        env.push(("OPENCODE_API_KEY".into(), "sk-test".into()));
        for (k, v) in env {
            // SAFETY: test-only single-threaded env mutation.
            unsafe { std::env::set_var(k, v) };
        }

        let c = OpenCodeClient::from_env();
        assert!(c.available());
        assert_eq!(c.base_url, "https://opencode.ai/zen/go/v1");
        assert_eq!(c.model, "deepseek-v4-flash");
    }
}
