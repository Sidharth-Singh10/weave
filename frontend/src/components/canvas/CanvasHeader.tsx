"use client";

import { useEffect, useMemo, useState } from "react";
import { MagnifyingGlass, Sparkle, X } from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";
import { searchGraph, type SearchResponse } from "@/lib/api";
import { useCommunityLabels } from "@/lib/useCommunityLabels";
import { usePersistenceStatus } from "@/lib/usePersistenceStatus";
import { TOPIC_COLORS } from "@/lib/graph-projection";
import type { ViewType } from "@/lib/graph-types";
import { UserMenu } from "@/components/auth/UserMenu";
import { Logo } from "@/components/ui/Logo";
import { SessionSwitcher } from "./SessionSwitcher";
import { ExportMenu } from "./ExportMenu";
import { CommonGroundBadge } from "./CommonGroundBadge";

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
    <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex flex-wrap items-center justify-between gap-2 border-b border-line bg-surface/85 px-3 py-2 backdrop-blur-md md:flex-nowrap md:px-4">
      <div className="flex items-center gap-2">
        <Logo size={22} />
        <CommonGroundBadge />
      </div>

      <GraphSearch />

      {isTopic && topics.length > 0 && (
        <div className="pointer-events-none hidden min-w-0 items-center gap-1.5 overflow-x-auto xl:flex">
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
        <div className="pointer-events-none hidden min-w-0 items-center gap-1.5 overflow-x-auto xl:flex">
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
        <div className="pointer-events-auto order-last flex min-w-0 items-center gap-1.5 rounded-xl border border-line bg-background/60 px-2.5 py-1 text-xs md:order-none">
          <button
            type="button"
            onClick={() => selectNode(undefined)}
            className="text-faint transition-colors hover:text-foreground"
          >
            Main Graph
          </button>
          <span className="text-faint">›</span>
          <span className="truncate font-medium text-muted">{selectedLabel}</span>
          <button
            type="button"
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

      <div className="pointer-events-auto flex items-center gap-2">
        <SessionSwitcher />
        <ExportMenu />
        <div className="flex items-center gap-0.5 rounded-xl border border-line bg-background/80 p-0.5">
          {VIEW_ORDER.map((v) => (
            <button
              key={v}
              type="button"
              onClick={() => setViewConfig({ type: v })}
              className={[
                "rounded-lg px-2.5 py-1 text-xs font-medium transition-colors sm:px-3",
                viewConfig.type === v
                  ? "bg-accent text-accent-ink"
                  : "text-muted hover:text-foreground",
              ].join(" ")}
            >
              {VIEW_LABELS[v]}
            </button>
          ))}
          <button
            type="button"
            onClick={() => void requestInsights()}
            title="Ask AI for suggestions"
            aria-label="Ask AI for suggestions"
            className={[
              "ml-1 flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-colors",
              hasInsights
                ? "bg-accent/15 text-accent"
                : "text-faint hover:text-foreground",
            ].join(" ")}
          >
            <Sparkle size={12} weight={insightsLoading ? "fill" : "regular"} />
            Suggest
          </button>
        </div>
        <UserMenu />
      </div>

      <QuotaBanner />
      <RestoredSessionToast />
    </div>
  );
}

/** Warning shown when localStorage refuses writes (quota exceeded). */
function QuotaBanner() {
  const quotaExceeded = usePersistenceStatus((s) => s.quotaExceeded);
  if (!quotaExceeded) return null;
  return (
    <div className="pointer-events-none absolute inset-x-0 top-full z-20 flex justify-center pt-2">
      <div className="pointer-events-auto rounded-xl border border-warning/40 bg-surface px-3 py-1.5 text-xs text-warning shadow-float">
        Storage full. The latest changes may not be saved.
      </div>
    </div>
  );
}

/** Brief notice after a saved session is restored on load. */
function RestoredSessionToast() {
  const restoredSession = usePersistenceStatus((s) => s.restoredSession);
  const clear = usePersistenceStatus((s) => s.setRestoredSession);

  useEffect(() => {
    if (!restoredSession) return;
    const t = setTimeout(() => clear(null), 3500);
    return () => clearTimeout(t);
  }, [restoredSession, clear]);

  if (!restoredSession) return null;
  return (
    <div className="pointer-events-none absolute inset-x-0 top-full z-20 flex justify-center pt-2">
      <div className="pointer-events-auto rounded-xl border border-line bg-surface px-3 py-1.5 text-xs text-muted shadow-float">
        Restored <span className="font-medium text-foreground">{restoredSession}</span>
      </div>
    </div>
  );
}

/** Hybrid semantic + LLM search over the graph, with explainable results. */
function GraphSearch() {
  const nodes = useGraphStore((s) => s.knowledgeNodes);
  const edges = useGraphStore((s) => s.knowledgeEdges);
  const selectNode = useGraphStore((s) => s.selectNode);
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<SearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);

  async function run(e?: React.FormEvent) {
    e?.preventDefault();
    const q = query.trim();
    if (!q) return;
    setLoading(true);
    try {
      const res = await searchGraph(
        q,
        nodes.map((n) => ({ label: n.label, kind: n.kind })),
        edges.map((e) => ({
          source_label:
            nodes.find((n) => n.id === e.source)?.label ?? e.source,
          target_label:
            nodes.find((n) => n.id === e.target)?.label ?? e.target,
          relation: e.relation,
        }))
      );
      setResult(res);
      setOpen(true);
    } catch {
      setResult(null);
      setOpen(false);
    } finally {
      setLoading(false);
    }
  }

  function jumpTo(label: string) {
    const node = nodes.find(
      (n) => n.label.toLowerCase() === label.toLowerCase()
    );
    if (node) {
      selectNode(node.id);
      setOpen(false);
      setQuery("");
    }
  }

  const highlight = new Set((result?.matches ?? []).map((m) => m.toLowerCase()));

  return (
    <div className="pointer-events-auto relative w-full min-w-0 sm:w-64">
      <form onSubmit={run} className="relative">
        <input
          value={query}
          aria-label="Search the graph"
          onChange={(e) => {
            setQuery(e.target.value);
            if (!e.target.value) setOpen(false);
          }}
          placeholder="Search the graph..."
          className="w-full rounded-xl border border-line bg-background/80 py-1 pl-7 pr-7 text-xs text-foreground outline-none transition-colors focus:border-accent"
        />
        <MagnifyingGlass
          size={13}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-faint"
        />
        {query && (
          <button
            type="button"
            onClick={() => {
              setQuery("");
              setOpen(false);
            }}
            className="absolute right-1.5 top-1/2 -translate-y-1/2 text-faint hover:text-foreground"
            aria-label="Clear search"
          >
            <X size={12} />
          </button>
        )}
      </form>

      {open && result && (
        <div className="absolute left-0 right-0 top-full z-30 mt-1 max-h-72 overflow-auto rounded-2xl border border-line bg-surface shadow-float">
          {loading ? (
            <p className="px-3 py-2 text-xs text-faint">Searching...</p>
          ) : (
            <>
              {result.rationale && (
                <p className="border-b border-line px-3 py-1.5 text-[11px] italic text-muted">
                  {result.rationale}
                </p>
              )}
              {(result.matches.length > 0 || result.semantic.length > 0) ? (
                <>
                  <ul className="py-1">
                    {result.matches.map((m) => (
                      <li key={m}>
                        <button
                          type="button"
                          onClick={() => jumpTo(m)}
                          className="flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-xs text-foreground transition-colors hover:bg-surface-2"
                        >
                          <span className="truncate">{m}</span>
                          {highlight.has(m.toLowerCase()) && (
                            <span className="shrink-0 rounded bg-accent/15 px-1.5 py-0.5 text-[10px] text-accent">
                              match
                            </span>
                          )}
                        </button>
                      </li>
                    ))}
                  </ul>
                  {result.semantic.length > 0 && (
                    <div className="border-t border-line px-3 py-1.5">
                      <p className="text-[10px] uppercase tracking-wide text-faint">
                        Semantically similar
                      </p>
                      {result.semantic.map((s) => (
                        <button
                          type="button"
                          key={s.label}
                          onClick={() => jumpTo(s.label)}
                          className="mt-0.5 flex w-full items-center justify-between gap-2 rounded px-1 py-1 text-left text-xs text-muted transition-colors hover:bg-surface-2 hover:text-foreground"
                        >
                          <span className="truncate">{s.label}</span>
                          <span className="shrink-0 font-mono text-[10px] text-faint">
                            {s.similarity.toFixed(2)}
                          </span>
                        </button>
                      ))}
                    </div>
                  )}
                </>
              ) : (
                <p className="px-3 py-2 text-xs text-faint">No matches.</p>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
