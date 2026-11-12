"use client";

import { useState } from "react";
import { Database, Fingerprint, ShieldCheck } from "@phosphor-icons/react";
import {
  MCP_CLAIM_EXAMPLE,
  MCP_CONNECT_HTTP,
  MCP_CONNECT_STDIO,
  MCP_RECALL_EXAMPLE,
  MCP_TOOL_GROUPS,
} from "@/lib/landing-content";
import { Badge } from "@/components/ui/Badge";
import { CodeBlock } from "@/components/ui/CodeBlock";
import { SegmentedControl } from "@/components/ui/SegmentedControl";
import { SectionHeading } from "@/components/ui/SectionHeading";

type MemoryTab = "connect" | "recall" | "claim";

const TABS: { value: MemoryTab; label: string }[] = [
  { value: "connect", label: "Connect" },
  { value: "recall", label: "Recall" },
  { value: "claim", label: "Claim" },
];

const LIFECYCLE = [
  {
    step: "01",
    title: "Remember",
    body: "A note is stored idempotently, then split into validated claims with evidence spans.",
  },
  {
    step: "02",
    title: "Validate",
    body: "Each claim gets a modality, confidence, status, and a resolution against existing entities.",
  },
  {
    step: "03",
    title: "Verify",
    body: "High-risk claims get a narrow structured review before they are committed.",
  },
  {
    step: "04",
    title: "Recall or correct",
    body: "Hybrid retrieval returns the smallest useful block. Corrections supersede, never overwrite.",
  },
];

export function MemorySection() {
  const [tab, setTab] = useState<MemoryTab>("connect");

  return (
    <section id="memory" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
          <SectionHeading
            title="A memory your agent can audit."
            lead="weave-mcp is a server-owned memory service for any MCP agent. Notes, files, and evidence-backed claims live in PostgreSQL with pgvector, exposed through 18 tools."
          />
          <div className="shrink-0 border-l border-line pl-5">
            <div className="font-display text-4xl font-semibold tracking-[-0.05em]">
              18
            </div>
            <p className="mt-1 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              MCP tools
            </p>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
          <div className="min-w-0 rounded-2xl border border-line bg-surface p-4 shadow-panel">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <SegmentedControl
                label="MCP example"
                value={tab}
                onChange={setTab}
                options={TABS}
              />
              <Badge tone="neutral">postgres + pgvector</Badge>
            </div>

            {tab === "connect" ? (
              <div className="space-y-3">
                <CodeBlock
                  name="stdio: claude desktop, codex, hermes"
                  code={MCP_CONNECT_STDIO}
                  maxHeight="14rem"
                />
                <CodeBlock
                  name="streamable http: containerized agents"
                  code={MCP_CONNECT_HTTP}
                  maxHeight="8rem"
                />
              </div>
            ) : (
              <CodeBlock
                name={tab === "recall" ? "recall_memory output" : "get_claim output"}
                code={tab === "recall" ? MCP_RECALL_EXAMPLE : MCP_CLAIM_EXAMPLE}
                maxHeight="24rem"
              />
            )}
          </div>

          <div className="rounded-2xl border border-line bg-surface p-4">
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              the contract
            </p>
            <div className="mt-3 divide-y divide-line">
              {MCP_TOOL_GROUPS.map((group) => (
                <div key={group.group} className="py-2.5 first:pt-0 last:pb-0">
                  <div className="flex items-center justify-between gap-2">
                    <h3 className="text-xs font-medium text-foreground">
                      {group.group}
                    </h3>
                    <span className="font-mono text-[10px] text-faint">
                      {group.tools.length}
                    </span>
                  </div>
                  <p className="mt-0.5 text-[11px] leading-relaxed text-faint">
                    {group.summary}
                  </p>
                  <div className="mt-1.5 flex flex-wrap gap-1">
                    {group.tools.map((tool) => (
                      <span
                        key={tool.name}
                        title={tool.description}
                        className="rounded-md border border-line bg-background px-1.5 py-0.5 font-mono text-[10px] text-muted"
                      >
                        {tool.name}
                      </span>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-1 gap-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4">
          {LIFECYCLE.map((item) => (
            <div key={item.step} className="bg-background p-4">
              <div className="flex items-center justify-between">
                <span className="font-mono text-[10px] tracking-[0.14em] text-faint">
                  {item.step}
                </span>
                <span className="size-1.5 rounded-full bg-accent/70" />
              </div>
              <h3 className="mt-3 text-sm font-medium text-foreground">
                {item.title}
              </h3>
              <p className="mt-1 text-[11px] leading-relaxed text-muted">
                {item.body}
              </p>
            </div>
          ))}
        </div>

        <div className="mt-8 grid grid-cols-1 gap-6 border-t border-line pt-8 sm:grid-cols-3">
          {[
            {
              icon: Fingerprint,
              title: "Evidence first",
              body: "Claims preserve the span, source note, modality, confidence, and status. Contradictions are linked, not erased.",
            },
            {
              icon: Database,
              title: "Per-agent isolation",
              body: "Each agent gets its own database and data volume, so no agent can read another agent's notes or files.",
            },
            {
              icon: ShieldCheck,
              title: "Audited lifecycle",
              body: "Writes, corrections, forgetting, and pruning record actor, action, and before and after values.",
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
