//! Rate-limit / quota policy data model.
//!
//! Policies are stored normalized in `rate_limit_policies` +
//! `rate_limit_rules`; this module provides the JSON-facing [`Limits`]
//! structure and its mapping to rule rows. Policy *resolution* (global → role
//! → user override, hard ceilings) lives in the rate-limit module.

use serde::{Deserialize, Serialize};

pub const METRIC_REQUESTS: &str = "requests";
pub const METRIC_TOKENS: &str = "tokens";
pub const METRIC_CONCURRENT: &str = "concurrent";

/// A (metric, optional window, limit) rule row.
#[derive(Debug, Clone)]
pub struct Rule {
    pub metric: &'static str,
    pub time_window: Option<&'static str>,
    pub limit: i64,
}

/// The human-facing limit set. `None` means "no rule for this slot" (inherit
/// or unset). Field names match the admin API contract in the spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_hour: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_day: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_day: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_month: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrent_requests: Option<i64>,
}

impl Limits {
    /// The rule rows this limit set maps to (excluding `None` fields).
    pub fn rules(&self) -> Vec<Rule> {
        let mut out = Vec::new();
        let mut push = |metric: &'static str, window: Option<&'static str>, v: &Option<i64>| {
            if let Some(v) = v {
                out.push(Rule {
                    metric,
                    time_window: window,
                    limit: *v,
                });
            }
        };
        push(METRIC_REQUESTS, Some("minute"), &self.requests_per_minute);
        push(METRIC_REQUESTS, Some("hour"), &self.requests_per_hour);
        push(METRIC_REQUESTS, Some("day"), &self.requests_per_day);
        push(METRIC_TOKENS, Some("minute"), &self.tokens_per_minute);
        push(METRIC_TOKENS, Some("day"), &self.tokens_per_day);
        push(METRIC_TOKENS, Some("month"), &self.tokens_per_month);
        push(METRIC_CONCURRENT, None, &self.concurrent_requests);
        out
    }

    /// Build a limit set from raw rule rows.
    pub fn from_rules(rules: &[RawRule]) -> Self {
        let mut limits = Limits::default();
        for r in rules {
            let slot: &mut Option<i64> = match (r.metric.as_str(), r.time_window.as_deref()) {
                (METRIC_REQUESTS, Some("minute")) => &mut limits.requests_per_minute,
                (METRIC_REQUESTS, Some("hour")) => &mut limits.requests_per_hour,
                (METRIC_REQUESTS, Some("day")) => &mut limits.requests_per_day,
                (METRIC_TOKENS, Some("minute")) => &mut limits.tokens_per_minute,
                (METRIC_TOKENS, Some("day")) => &mut limits.tokens_per_day,
                (METRIC_TOKENS, Some("month")) => &mut limits.tokens_per_month,
                (METRIC_CONCURRENT, None) => &mut limits.concurrent_requests,
                _ => continue,
            };
            *slot = Some(r.limit);
        }
        limits
    }

    pub fn is_empty(&self) -> bool {
        self == &Limits::default()
    }
}

/// A raw rule row read from `rate_limit_rules`.
#[derive(Debug, Clone)]
pub struct RawRule {
    pub metric: String,
    pub time_window: Option<String>,
    pub limit: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip() {
        let limits = Limits {
            requests_per_minute: Some(30),
            tokens_per_day: Some(500_000),
            concurrent_requests: Some(2),
            ..Limits::default()
        };
        let rules = limits.rules();
        assert_eq!(rules.len(), 3);

        let raw: Vec<RawRule> = rules
            .into_iter()
            .map(|r| RawRule {
                metric: r.metric.to_string(),
                time_window: r.time_window.map(|w| w.to_string()),
                limit: r.limit,
            })
            .collect();
        assert_eq!(Limits::from_rules(&raw), limits);
    }

    #[test]
    fn empty_limits_have_no_rules() {
        assert!(Limits::default().rules().is_empty());
        assert!(Limits::default().is_empty());
    }

    #[test]
    fn concurrent_has_no_window() {
        let limits = Limits {
            concurrent_requests: Some(5),
            ..Limits::default()
        };
        let rules = limits.rules();
        assert_eq!(rules[0].metric, METRIC_CONCURRENT);
        assert_eq!(rules[0].time_window, None);
    }
}
