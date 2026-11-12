"use client";

import { useState } from "react";
import { ArrowRight, Gauge, ShieldCheck, WarningCircle } from "@phosphor-icons/react";
import {
  ARCHITECTURE_LANES,
  PLATFORM_ERROR_EXAMPLE,
  PLATFORM_POLICY_EXAMPLE,
  PLATFORM_STAGES,
} from "@/lib/landing-content";
import { Badge } from "@/components/ui/Badge";
import { CodeBlock } from "@/components/ui/CodeBlock";
import { SectionHeading } from "@/components/ui/SectionHeading";

const TOTAL_MS = PLATFORM_STAGES.reduce(
  (total, stage) => total + stage.durationMs,
  0
);

export function PlatformSection() {
  const [activeId, setActiveId] = useState("extract");
  const stage =
    PLATFORM_STAGES.find((item) => item.id === activeId) ?? PLATFORM_STAGES[6];

  return (
    <section id="platform" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
          <SectionHeading
            title="Governance, not guesswork."
            lead="Authentication, permissions, rate limits, quotas, concurrency, usage, and audit are one trace. An admin can see every ingest stage, including the exact prompt and the subgraph that reached the model."
          />
          <div className="shrink-0 border-l border-line pl-5">
            <div className="font-display text-4xl font-semibold tracking-[-0.05em]">
              {PLATFORM_STAGES.length}
            </div>
            <p className="mt-1 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              traced stages
            </p>
          </div>
        </div>

        <div className="mt-8 overflow-hidden rounded-2xl border border-line bg-surface shadow-panel">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-3">
            <div className="flex items-center gap-2 font-mono text-[11px] text-faint">
              <span className="size-1.5 rounded-full bg-accent" />
              <span className="text-muted">POST /api/graph/ingest</span>
              <span className="text-faint">observe only</span>
            </div>
            <div className="flex items-center gap-2">
              <Badge tone="neutral">{TOTAL_MS} ms</Badge>
              <Badge tone="accent">X-Request-ID</Badge>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
            <ol className="border-b border-line p-2 lg:border-b-0 lg:border-r">
              {PLATFORM_STAGES.map((item) => {
                const active = item.id === activeId;
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      aria-pressed={active}
                      onClick={() => setActiveId(item.id)}
                      className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors ${
                        active
                          ? "bg-accent/[0.08] text-foreground"
                          : "text-muted hover:bg-surface-2 hover:text-foreground"
                      }`}
                    >
                      <span
                        className={`size-1.5 shrink-0 rounded-full ${
                          item.status === "info" ? "bg-warning" : "bg-accent/70"
                        }`}
                      />
                      <span className="min-w-0 flex-1 truncate text-sm">
                        {item.label}
                      </span>
                      <span
                        className={`shrink-0 font-mono text-[10px] ${
                          active ? "text-muted" : "text-faint"
                        }`}
                      >
                        {item.durationMs} ms
                      </span>
                    </button>
                  </li>
                );
              })}
            </ol>

            <div className="min-w-0 p-5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h3 className="font-display text-xl font-semibold tracking-[-0.03em]">
                  {stage.label}
                </h3>
                <Badge tone={stage.status === "info" ? "warning" : "accent"}>
                  {stage.status === "info" ? "model or parser" : "ok"}
                </Badge>
              </div>
              <p className="mt-1 text-xs text-faint">{stage.summary}</p>
              <p className="mt-3 text-sm leading-relaxed text-muted">
                {stage.detail}
              </p>
              {stage.code && (
                <div className="mt-4">
                  <CodeBlock
                    name={stage.codeName ?? "stage detail"}
                    code={stage.code}
                    maxHeight="15rem"
                  />
                </div>
              )}
            </div>
          </div>
        </div>

        <div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <div className="rounded-2xl border border-line bg-surface p-4">
            <div className="flex items-center gap-2">
              <Gauge size={16} className="text-accent" />
              <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                policy inheritance
              </p>
            </div>
            <p className="mt-2 text-xs leading-relaxed text-muted">
              Global, role, and user values resolve from least to most specific.
              A hard ceiling always wins.
            </p>
            <div className="mt-3">
              <CodeBlock
                name="effective policy"
                code={PLATFORM_POLICY_EXAMPLE}
                maxHeight="12rem"
              />
            </div>
          </div>

          <div className="rounded-2xl border border-line bg-surface p-4">
            <div className="flex items-center gap-2">
              <WarningCircle size={16} className="text-warning" />
              <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                error contract
              </p>
            </div>
            <p className="mt-2 text-xs leading-relaxed text-muted">
              Blocked requests return a standardized error, a request id, and
              Retry-After. Rate-limit failures never silently allow traffic.
            </p>
            <div className="mt-3">
              <CodeBlock
                name="429 response"
                code={PLATFORM_ERROR_EXAMPLE}
                maxHeight="10rem"
              />
            </div>
          </div>
        </div>

        <div className="mt-4 rounded-2xl border border-line bg-surface p-5">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <div className="flex items-center gap-2">
                <ShieldCheck size={16} className="text-accent" />
                <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                  architecture
                </p>
              </div>
              <h3 className="mt-2 font-display text-xl font-semibold tracking-[-0.03em]">
                One origin. Two kinds of memory.
              </h3>
            </div>
            <p className="max-w-md text-xs leading-relaxed text-muted">
              The canvas graph is client-owned. The agent memory is server-owned.
              The API never persists graph contents, and the two do not share a
              database.
            </p>
          </div>

          <div className="mt-5 grid grid-cols-1 gap-px border border-line bg-line md:grid-cols-4">
            {ARCHITECTURE_LANES.map((lane) => (
              <div key={lane.label} className="bg-background p-4">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                    {lane.label}
                  </span>
                  <ArrowRight size={12} className="text-line-bright" />
                </div>
                <h4 className="mt-3 text-sm font-medium text-foreground">
                  {lane.title}
                </h4>
                <p className="mt-1.5 text-[11px] leading-relaxed text-muted">
                  {lane.detail}
                </p>
                <div className="mt-3 flex flex-wrap gap-1">
                  {lane.items.map((item) => (
                    <span
                      key={item}
                      className="rounded-md border border-line px-1.5 py-0.5 font-mono text-[10px] text-faint"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
