"use client";

import { useState } from "react";
import {
  BracketsCurly,
  BoundingBox,
  SealCheck,
} from "@phosphor-icons/react";
import {
  EXTRACTION_EXAMPLES,
  type ExtractionExample,
} from "@/lib/landing-content";
import { Badge } from "@/components/ui/Badge";
import { SegmentedControl } from "@/components/ui/SegmentedControl";
import { SectionHeading } from "@/components/ui/SectionHeading";
import { GraphStage } from "./GraphStage";

const FEATURES = [
  {
    icon: BracketsCurly,
    title: "Parse first",
    body: "A Markdown outline or JSON document that states its own graph is handled in-process. Same input, same output, no provider call.",
  },
  {
    icon: BoundingBox,
    title: "Bound the model",
    body: "Prose gets the relevant two-hop neighborhood, capped at 64 nodes and 256 edges, so ingest cost stops scaling with the graph.",
  },
  {
    icon: SealCheck,
    title: "Keep the proof",
    body: "Every edge keeps the sentence that supports it and a grounding class: extracted, inferred, or ambiguous.",
  },
] as const;

export function ExtractionSection() {
  const [selected, setSelected] =
    useState<ExtractionExample["id"]>("outline");
  const example =
    EXTRACTION_EXAMPLES.find((item) => item.id === selected) ??
    EXTRACTION_EXAMPLES[0];

  return (
    <section id="extraction" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <div className="grid grid-cols-1 gap-10 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] lg:gap-14">
          <div>
            <SectionHeading
              title="Not every note needs a model."
              lead="Structured notes are parsed directly. Prose goes to a bounded model call. Both paths keep the evidence, and the tracer shows which one ran."
            />

            <SegmentedControl
              label="Choose an extraction example"
              className="mt-7"
              value={selected}
              onChange={setSelected}
              options={[
                { value: "outline", label: "Structured note" },
                { value: "prose", label: "Prose note" },
              ]}
            />

            <div className="mt-8 space-y-5">
              {FEATURES.map((feature) => (
                <div key={feature.title} className="flex gap-4">
                  <feature.icon size={18} className="mt-0.5 shrink-0 text-accent" />
                  <div>
                    <h3 className="text-sm font-medium text-foreground">
                      {feature.title}
                    </h3>
                    <p className="mt-1 text-sm leading-relaxed text-muted">
                      {feature.body}
                    </p>
                  </div>
                </div>
              ))}
            </div>

            <dl className="mt-8 grid grid-cols-3 gap-px border border-line bg-line">
              {[
                ["60%", "structure gate"],
                ["64", "node cap"],
                ["256", "edge cap"],
              ].map(([value, label]) => (
                <div key={label} className="bg-background px-3 py-3">
                  <dt className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                    {label}
                  </dt>
                  <dd className="mt-1 font-display text-xl font-semibold tracking-[-0.03em]">
                    {value}
                  </dd>
                </div>
              ))}
            </dl>
          </div>

          <div className="overflow-hidden rounded-2xl border border-line bg-surface shadow-panel">
            <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-4 py-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className="size-1.5 shrink-0 rounded-full bg-accent" />
                <span className="truncate font-mono text-[11px] text-muted">
                  {example.file}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <Badge tone={example.tier === "deterministic" ? "accent" : "neutral"}>
                  {example.tier === "deterministic" ? "no model call" : "bounded LLM"}
                </Badge>
                <span className="font-mono text-[11px] text-faint">
                  {example.tokens} tokens
                </span>
              </div>
            </div>

            <div className="border-b border-line bg-[#0a0c0f] px-4 py-3">
              <pre className="weave-scroll max-h-40 overflow-auto font-mono text-[11.5px] leading-relaxed text-muted">
                <code>{example.note}</code>
              </pre>
            </div>

            <div className="relative h-[290px] bg-background/40">
              <GraphStage
                key={example.id}
                nodes={example.graph.nodes}
                edges={example.graph.edges}
                level={3}
                label={`Graph produced from the ${example.label.toLowerCase()}`}
              />
            </div>

            <div className="border-t border-line px-4 py-3">
              <p className="text-xs leading-relaxed text-muted">{example.summary}</p>
              <div className="mt-3 flex items-start gap-2 border-l border-accent/40 pl-3">
                <SealCheck size={14} className="mt-0.5 shrink-0 text-accent" />
                <div className="min-w-0">
                  <p className="truncate font-mono text-[11px] text-foreground/90">
                    {example.evidence}
                  </p>
                  <div className="mt-1.5 flex items-center gap-2">
                    <Badge
                      tone={
                        example.confidence === "EXTRACTED"
                          ? "accent"
                          : example.confidence === "INFERRED"
                            ? "neutral"
                            : "warning"
                      }
                    >
                      {example.confidence}
                    </Badge>
                    <span className="text-[11px] text-faint">
                      evidence retained from the note
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
