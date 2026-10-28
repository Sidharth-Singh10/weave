"use client";

import { useMemo } from "react";

import { useGraphStore } from "@/lib/store";
import { TOPIC_COLORS } from "@/lib/graph-projection";
import type { EdgeConfidence, SourceRef } from "@/lib/graph-types";

/** Confidence badge colors, matching how edges are drawn on the canvas. */
const CONFIDENCE_COLORS: Record<EdgeConfidence, string> = {
  EXTRACTED: "var(--accent)",
  INFERRED: "var(--muted)",
  AMBIGUOUS: "#f59e0b",
};

/** Shown for edges persisted before confidence was recorded. */
const UNRECORDED_COLOR = "var(--faint)";

function confidenceLabel(confidence: EdgeConfidence | undefined): string {
  return confidence ? confidence.toLowerCase() : "unrecorded";
}

const MAX_CONNECTIONS = 8;

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function SourceRow({ source }: { source: SourceRef }) {
  if (!source.note) {
    return <p className="text-xs text-faint">Manually created</p>;
  }
  return (
    <div>
      <p className="text-xs leading-relaxed text-muted">“{source.note}”</p>
      <p className="mt-0.5 text-[11px] text-faint">{formatTime(source.createdAt)}</p>
    </div>
  );
}

/**
 * Side panel showing provenance (originating notes) for the focused node, plus
 * its relationships with the confidence class each edge was grounded under.
 * Deterministic — computed from the knowledge graph, no LLM call.
 */
export function SourceInspector() {
  const selectedNodeId = useGraphStore((s) => s.viewConfig.selectedNodeId);
  const node = useGraphStore((s) =>
    s.knowledgeNodes.find((n) => n.id === s.viewConfig.selectedNodeId)
  );
  const source = useGraphStore((s) =>
    s.viewConfig.selectedNodeId
      ? s.nodeSources[s.viewConfig.selectedNodeId]
      : undefined
  );
  const nodes = useGraphStore((s) => s.knowledgeNodes);
  const edges = useGraphStore((s) => s.knowledgeEdges);
  const selectNode = useGraphStore((s) => s.selectNode);

  const connections = useMemo(() => {
    if (!selectedNodeId) return [];
    const byId = new Map(nodes.map((n) => [n.id, n] as const));
    return edges
      .filter((e) => e.source === selectedNodeId || e.target === selectedNodeId)
      .map((e) => {
        const outgoing = e.source === selectedNodeId;
        const other = byId.get(outgoing ? e.target : e.source);
        return {
          id: e.id,
          outgoing,
          otherLabel: other?.label ?? "unknown",
          relation: e.relation,
          confidence: e.confidence,
          evidence: e.evidence,
        };
      });
  }, [edges, nodes, selectedNodeId]);

  if (!selectedNodeId || !node) return null;

  const shown = connections.slice(0, MAX_CONNECTIONS);
  const hidden = connections.length - shown.length;

  return (
    <div className="pointer-events-auto absolute right-3 top-14 z-10 w-72 rounded-xl border border-line bg-surface p-3 shadow-lg">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium text-foreground">{node.label}</p>
          <p className="text-[11px] text-faint">{node.kind}</p>
        </div>
        <button
          onClick={() => selectNode(undefined)}
          className="shrink-0 text-faint transition-colors hover:text-foreground"
          aria-label="Close inspector"
        >
          ✕
        </button>
      </div>

      <div className="mt-3 space-y-2">
        <p className="text-[11px] font-medium uppercase tracking-wide text-faint">
          Source
        </p>
        {source ? (
          <SourceRow source={source} />
        ) : (
          <p className="text-xs text-faint">No source recorded.</p>
        )}
      </div>

      <div className="mt-3 space-y-1.5">
        <p className="text-[11px] font-medium uppercase tracking-wide text-faint">
          Connections ({connections.length})
        </p>
        {connections.length === 0 ? (
          <p className="text-xs text-faint">No relationships yet.</p>
        ) : (
          <ul className="max-h-64 space-y-1.5 overflow-y-auto pr-0.5">
            {shown.map((c) => (
              <li
                key={c.id}
                className="rounded-lg border border-line px-2 py-1.5"
                title={c.evidence ?? undefined}
              >
                <div className="flex items-baseline gap-1.5 text-xs">
                  <span className="shrink-0 text-faint" aria-hidden>
                    {c.outgoing ? "→" : "←"}
                  </span>
                  <span className="truncate text-foreground">{c.otherLabel}</span>
                </div>
                <div className="mt-0.5 flex items-center justify-between gap-2">
                  <span className="truncate text-[11px] text-muted">{c.relation}</span>
                  <span
                    className="shrink-0 text-[10px] uppercase tracking-wide"
                    style={{
                      color: c.confidence
                        ? CONFIDENCE_COLORS[c.confidence]
                        : UNRECORDED_COLOR,
                    }}
                  >
                    {confidenceLabel(c.confidence)}
                  </span>
                </div>
                {c.evidence ? (
                  <p className="mt-1 text-[10px] leading-snug text-faint">
                    “{c.evidence}”
                  </p>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {hidden > 0 ? (
          <p className="text-[11px] text-faint">+{hidden} more</p>
        ) : null}
      </div>

      <div className="mt-2 flex items-center gap-1.5">
        <span
          className="size-1.5 rounded-full"
          style={{ backgroundColor: TOPIC_COLORS[node.kind] ?? "#a1a1aa" }}
        />
        <span className="text-[11px] text-faint">Topic view tint</span>
      </div>
    </div>
  );
}
