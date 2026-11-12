"use client";

import { useState } from "react";
import { ArrowsMerge, MagnifyingGlass, Ranking } from "@phosphor-icons/react";
import {
  CANVAS_GRAPH,
  RETRIEVAL_QUERIES,
  type RetrievalResult,
} from "@/lib/landing-content";
import { Badge } from "@/components/ui/Badge";
import { SectionHeading } from "@/components/ui/SectionHeading";
import { GraphStage } from "./GraphStage";

const CHANNEL_TONE = {
  lexical: "neutral",
  semantic: "accent",
  "graph path": "outline",
} as const;

export function RetrievalSection() {
  const [queryId, setQueryId] = useState(RETRIEVAL_QUERIES[0].id);
  const query =
    RETRIEVAL_QUERIES.find((item) => item.id === queryId) ??
    RETRIEVAL_QUERIES[0];
  const [selected, setSelected] = useState<RetrievalResult>(query.results[0]);

  function chooseQuery(id: (typeof RETRIEVAL_QUERIES)[number]["id"]) {
    const next = RETRIEVAL_QUERIES.find((item) => item.id === id);
    if (!next) return;
    setQueryId(id);
    setSelected(next.results[0]);
  }

  return (
    <section id="retrieval" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <SectionHeading
          title="Ask for the thread, not the file."
          lead="Hybrid retrieval ranks by words and by meaning, expands through the graph, then shows why each result matched. Selecting a result jumps to the concept."
        />

        <div className="mt-8 grid grid-cols-1 overflow-hidden rounded-2xl border border-line bg-surface shadow-panel lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
          <div className="border-b border-line p-5 lg:border-b-0 lg:border-r">
            <div className="flex items-center gap-2">
              <MagnifyingGlass size={16} className="text-accent" />
              <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-faint">
                example retrieval
              </p>
            </div>

            <div className="mt-4 space-y-1.5">
              {RETRIEVAL_QUERIES.map((item) => {
                const active = item.id === queryId;
                return (
                  <button
                    key={item.id}
                    type="button"
                    aria-pressed={active}
                    onClick={() => chooseQuery(item.id)}
                    className={`flex w-full items-center justify-between gap-3 rounded-xl border px-3 py-2.5 text-left text-xs transition-colors ${
                      active
                        ? "border-accent/40 bg-accent/[0.08] text-foreground"
                        : "border-line bg-background/50 text-muted hover:border-line-bright hover:text-foreground"
                    }`}
                  >
                    <span className="truncate">{item.query}</span>
                    <span
                      className={`shrink-0 font-mono text-[10px] ${
                        active ? "text-muted" : "text-faint"
                      }`}
                    >
                      {active ? "active" : "run"}
                    </span>
                  </button>
                );
              })}
            </div>

            <div className="mt-5 flex items-center justify-between border-t border-line pt-4">
              <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                ranked results
              </p>
              <span className="text-[10px] text-faint">example data</span>
            </div>

            <ul className="mt-2 space-y-1.5">
              {query.results.map((result, index) => {
                const active = result.nodeId === selected.nodeId;
                return (
                  <li key={`${query.id}-${result.nodeId}`}>
                    <button
                      type="button"
                      aria-pressed={active}
                      onClick={() => setSelected(result)}
                      className={`w-full rounded-xl border px-3 py-2.5 text-left transition-colors ${
                        active
                          ? "border-line-bright bg-surface-2"
                          : "border-transparent hover:bg-surface-2/70"
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="font-mono text-[10px] text-faint">
                            {String(index + 1).padStart(2, "0")}
                          </span>
                          <span className="truncate text-sm text-foreground">
                            {result.label}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-2">
                          <Badge tone={CHANNEL_TONE[result.channel]}>
                            {result.channel}
                          </Badge>
                          <span className="font-mono text-[11px] text-muted">
                            {result.score.toFixed(2)}
                          </span>
                        </span>
                      </div>
                      <p className="mt-1 pl-6 text-[11px] leading-relaxed text-faint">
                        {result.why}
                      </p>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>

          <div className="flex flex-col">
            <div className="flex items-center justify-between border-b border-line px-4 py-3">
              <p className="font-mono text-[11px] text-faint">
                highlighting <span className="text-foreground">{selected.label}</span>
              </p>
              <span className="font-mono text-[10px] text-faint">
                graph + text + vectors
              </span>
            </div>
            <div className="h-[340px] flex-1 bg-background/40 md:h-[420px]">
              <GraphStage
                nodes={CANVAS_GRAPH.nodes}
                edges={CANVAS_GRAPH.edges}
                level={2}
                activeIds={[selected.nodeId]}
                label={`Retrieval result ${selected.label} highlighted in the graph`}
              />
            </div>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-1 gap-6 border-t border-line pt-8 sm:grid-cols-3">
          {[
            {
              icon: ArrowsMerge,
              title: "Merge, don't replace",
              body: "Lexical matches, embeddings, graph expansion, and claims are merged with reciprocal-rank fusion.",
            },
            {
              icon: Ranking,
              title: "Explain the ranking",
              body: "Each result carries a score and a plain-language reason, so a surprising match can be inspected.",
            },
            {
              icon: MagnifyingGlass,
              title: "Local embeddings",
              body: "The API embeds the graph with a local ONNX model. No third-party embedding call is required.",
            },
          ].map((item) => (
            <div key={item.title} className="flex gap-3">
              <item.icon size={18} className="mt-0.5 shrink-0 text-accent" />
              <div>
                <h3 className="text-sm font-medium text-foreground">
                  {item.title}
                </h3>
                <p className="mt-1 text-sm leading-relaxed text-muted">
                  {item.body}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
