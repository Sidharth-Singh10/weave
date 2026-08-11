"use client";

import { useMemo } from "react";
import { Sparkle } from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";
import { useCommunityLabels } from "@/lib/useCommunityLabels";
import { TOPIC_COLORS } from "@/lib/graph-projection";
import type { ViewType } from "@/lib/graph-types";

const VIEW_LABELS: Record<ViewType, string> = {
  default: "Default",
  topic: "Topic",
};

const VIEW_ORDER: ViewType[] = ["default", "topic"];

/** Compact overlay at the top of the canvas: brand mark + breadcrumb + view tabs. */
export function CanvasHeader() {
  const viewConfig = useGraphStore((s) => s.viewConfig);
  const knowledgeNodes = useGraphStore((s) => s.knowledgeNodes);
  const selectNode = useGraphStore((s) => s.selectNode);
  const setViewConfig = useGraphStore((s) => s.setViewConfig);
  const requestInsights = useGraphStore((s) => s.requestInsights);
  const insightsLoading = useGraphStore((s) => s.insightsLoading);
  const hasInsights = useGraphStore((s) => s.insights !== null);
  const communities = useCommunityLabels();

  const isTopic = viewConfig.type === "topic";

  const topics = useMemo(() => {
    if (!isTopic) return [];
    const seen = new Map<string, string>();
    for (const n of knowledgeNodes) {
      if (!seen.has(n.kind)) seen.set(n.kind, TOPIC_COLORS[n.kind] ?? "#a1a1aa");
    }
    return Array.from(seen.entries());
  }, [isTopic, knowledgeNodes]);

  const selectedLabel = viewConfig.selectedNodeId
    ? knowledgeNodes.find((n) => n.id === viewConfig.selectedNodeId)?.label
    : undefined;

  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-center justify-between gap-4 border-b border-line bg-surface/80 px-4 py-2 backdrop-blur-md">
      <span className="text-sm font-semibold tracking-tight text-foreground">
        Weave
      </span>

      {isTopic && topics.length > 0 && (
        <div className="pointer-events-none flex min-w-0 items-center gap-1.5 overflow-x-auto">
          {topics.map(([kind, color]) => (
            <span
              key={kind}
              className="flex shrink-0 items-center gap-1.5 rounded-full border border-line bg-background/60 px-2 py-0.5 text-[11px] text-muted"
            >
              <span
                className="size-1.5 rounded-full"
                style={{ backgroundColor: color }}
              />
              {kind}
            </span>
          ))}
        </div>
      )}

      {!isTopic && communities.length > 0 && (
        <div className="pointer-events-none flex min-w-0 items-center gap-1.5 overflow-x-auto">
          {communities.map((c) => (
            <span
              key={c.signature}
              className="flex shrink-0 items-center gap-1.5 rounded-full border border-line bg-background/60 px-2 py-0.5 text-[11px] text-muted"
            >
              <span
                className="size-1.5 rounded-full"
                style={{ backgroundColor: c.color }}
              />
              {c.label}
            </span>
          ))}
        </div>
      )}

      {selectedLabel && (
        <div className="pointer-events-auto flex min-w-0 items-center gap-1.5 rounded-lg border border-line bg-background/60 px-2.5 py-1 text-xs">
          <button
            onClick={() => selectNode(undefined)}
            className="text-faint transition-colors hover:text-foreground"
          >
            Main Graph
          </button>
          <span className="text-faint">›</span>
          <span className="truncate font-medium text-muted">{selectedLabel}</span>
          <button
            title="Expand neighborhood"
            aria-label="Expand neighborhood"
            onClick={() =>
              setViewConfig({
                focusDepth: (viewConfig.focusDepth ?? 1) + 1,
              })
            }
            className="ml-1 grid size-4 place-items-center rounded bg-surface-2 text-muted transition-colors hover:text-foreground"
          >
            +
          </button>
        </div>
      )}

      <div className="pointer-events-auto flex items-center gap-0.5 rounded-lg border border-line bg-background/80 p-0.5">
        {VIEW_ORDER.map((v) => (
          <button
            key={v}
            onClick={() => setViewConfig({ type: v })}
            className={[
              "rounded-md px-3 py-1 text-xs font-medium transition-colors",
              viewConfig.type === v
                ? "bg-accent text-accent-ink"
                : "text-muted hover:text-foreground",
            ].join(" ")}
          >
            {VIEW_LABELS[v]}
          </button>
        ))}
        <button
          onClick={() => void requestInsights()}
          title="Ask AI for suggestions"
          aria-label="Ask AI for suggestions"
          className={[
            "ml-1 flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-colors",
            hasInsights
              ? "bg-accent/15 text-accent"
              : "text-faint hover:text-foreground",
          ].join(" ")}
        >
          <Sparkle size={12} weight={insightsLoading ? "fill" : "regular"} />
          Suggest
        </button>
      </div>
    </div>
  );
}
