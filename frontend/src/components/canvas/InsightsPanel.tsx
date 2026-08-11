"use client";

import { X, Sparkle } from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";

/** Advisory AI analysis of the graph. Read-only suggestions — the AI never
 * mutates knowledge or controls the canvas directly. */
export function InsightsPanel() {
  const insights = useGraphStore((s) => s.insights);
  const loading = useGraphStore((s) => s.insightsLoading);
  const clearInsights = useGraphStore((s) => s.clearInsights);

  if (!insights && !loading) return null;

  const empty =
    insights &&
    insights.groups.length === 0 &&
    insights.missing_edges.length === 0 &&
    insights.disconnected.length === 0 &&
    insights.duplicates.length === 0;

  return (
    <div className="pointer-events-auto absolute bottom-6 left-4 z-10 w-72 max-w-[calc(100vw-2rem)] rounded-2xl border border-line bg-surface/95 p-3 shadow-[0_16px_60px_rgba(0,0,0,0.5)] backdrop-blur-md">
      <div className="mb-2 flex items-center justify-between">
        <span className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
          <Sparkle size={14} className="text-accent" />
          AI Suggestions
        </span>
        <button
          onClick={clearInsights}
          aria-label="Close suggestions"
          className="text-faint transition-colors hover:text-foreground"
        >
          <X size={14} />
        </button>
      </div>

      {loading ? (
        <p className="text-xs text-faint">Reading the graph…</p>
      ) : empty ? (
        <p className="text-xs text-faint">
          No suggestions right now — the graph looks well organized.
        </p>
      ) : (
        <div className="max-h-72 space-y-2 overflow-y-auto text-xs">
          {insights!.groups.length > 0 && (
            <div>
              <p className="mb-1 font-medium text-muted">Suggested groups</p>
              <ul className="space-y-1 text-foreground/90">
                {insights!.groups.map((g, i) => (
                  <li key={i} className="rounded-lg bg-surface-2 px-2 py-1.5">
                    <span className="font-medium text-accent">{g.label}</span>
                    <span className="ml-1 text-faint">
                      — {g.member_labels.join(", ")}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {insights!.missing_edges.length > 0 && (
            <div>
              <p className="mb-1 font-medium text-muted">Possible missing edges</p>
              <ul className="space-y-1 text-foreground/90">
                {insights!.missing_edges.map((e, i) => (
                  <li key={i}>
                    {e.source_label} —{e.relation}→ {e.target_label}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {insights!.duplicates.length > 0 && (
            <div>
              <p className="mb-1 font-medium text-muted">Possible duplicates</p>
              <ul className="space-y-1 text-foreground/90">
                {insights!.duplicates.map((d, i) => (
                  <li key={i}>
                    {d.label_a} ≈ {d.label_b}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {insights!.disconnected.length > 0 && (
            <div>
              <p className="mb-1 font-medium text-muted">Disconnected nodes</p>
              <ul className="space-y-1 text-foreground/90">
                {insights!.disconnected.map((d, i) => (
                  <li key={i}>{d}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
