"use client";

import { useState } from "react";
import { ShareNetwork, Sparkle, TrendUp } from "@phosphor-icons/react";
import {
  CANVAS_EDGE,
  CANVAS_GRAPH,
  CANVAS_LEVELS,
  COMMUNITY_COLORS,
  type ZoomLevel,
} from "@/lib/landing-content";
import { Badge } from "@/components/ui/Badge";
import { SegmentedControl } from "@/components/ui/SegmentedControl";
import { SectionHeading } from "@/components/ui/SectionHeading";
import { GraphStage } from "./GraphStage";
import { useMediaQuery } from "@/lib/useMediaQuery";

const FEATURES = [
  {
    icon: TrendUp,
    title: "Importance",
    body: "Degree centrality makes hubs prominent. Peripheral concepts stay readable without competing for attention.",
  },
  {
    icon: ShareNetwork,
    title: "Communities",
    body: "Label propagation finds clusters, the model names them, and a plain fallback keeps the view usable when it cannot.",
  },
  {
    icon: Sparkle,
    title: "Suggestions",
    body: "Deterministic signals find hubs, bridges, isolated ideas, and near-duplicates. The model only names groups and proposes missing edges.",
  },
] as const;

const INSPECTOR_CONNECTIONS = [
  { relation: "designed by", other: "Charles Babbage", confidence: "EXTRACTED" as const },
  { relation: "used", other: "Punched cards", confidence: "INFERRED" as const },
  { relation: "wrote about by", other: "Menabrea", confidence: "EXTRACTED" as const },
];

export function CanvasSection() {
  const isMobile = useMediaQuery("(max-width: 767px)");
  const [selected, setSelected] = useState<ZoomLevel | null>(null);
  const active = selected ?? (isMobile ? "overview" : "detail");
  const canvasLevel =
    CANVAS_LEVELS.find((item) => item.value === active) ?? CANVAS_LEVELS[3];

  return (
    <section id="canvas" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <SectionHeading
            title="One graph. Four levels of attention."
            lead="Semantic zoom changes how much the graph tells you, not just how large it is. Hubs lead, communities guide, and every edge can explain itself."
          />
          <SegmentedControl
            label="Semantic zoom level"
            value={active}
            onChange={setSelected}
            options={CANVAS_LEVELS.map((item) => ({
              value: item.value,
              label: item.label,
            }))}
            className="shrink-0"
          />
        </div>

        <div className="mt-8 overflow-hidden rounded-2xl border border-line bg-surface shadow-panel">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-3">
            <div className="flex items-center gap-2 font-mono text-[11px] text-faint">
              <span className="size-1.5 rounded-full bg-accent" />
              <span className="text-muted">semantic zoom</span>
              <span className="text-line-bright">/</span>
              <span>{canvasLevel.label.toLowerCase()}</span>
            </div>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
              {Object.entries(CANVAS_GRAPH.communities ?? {}).map(
                ([id, name]) => (
                  <span
                    key={id}
                    className="inline-flex items-center gap-1.5 text-[10px] text-faint"
                  >
                    <span
                      className="size-1.5 rounded-full"
                      style={{
                        backgroundColor:
                          COMMUNITY_COLORS[Number(id) % COMMUNITY_COLORS.length],
                      }}
                    />
                    {name}
                  </span>
                )
              )}
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1.65fr)_minmax(0,1fr)]">
            <div className="h-[300px] border-b border-line bg-background/40 md:h-[440px] lg:border-b-0">
              <GraphStage
                nodes={CANVAS_GRAPH.nodes}
                edges={CANVAS_GRAPH.edges}
                communities={CANVAS_GRAPH.communities}
                level={canvasLevel.level}
                label={`Knowledge graph at the ${canvasLevel.label.toLowerCase()} level`}
              />
            </div>

            <aside className="p-5 lg:border-l lg:border-line">
              <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-accent">
                {canvasLevel.label}
              </p>
              <h3 className="mt-2 font-display text-xl font-semibold tracking-[-0.03em]">
                {canvasLevel.title}
              </h3>
              <p className="mt-2 text-sm leading-relaxed text-muted">
                {canvasLevel.body}
              </p>

              <div className="mt-6 border-t border-line pt-4">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                    selected edge
                  </span>
                  <Badge tone="accent">{CANVAS_EDGE.confidence}</Badge>
                </div>
                <p className="mt-2 text-sm text-foreground">
                  {CANVAS_EDGE.from}{" "}
                  <span className="text-faint">to</span> {CANVAS_EDGE.to}
                </p>
                <p className="mt-1 font-mono text-[11px] text-accent">
                  {CANVAS_EDGE.relation}
                </p>
                <blockquote className="mt-3 border-l border-line pl-3 text-xs leading-relaxed text-muted">
                  {CANVAS_EDGE.evidence}
                </blockquote>
                <p className="mt-2 text-[10px] text-faint">
                  Source note: {CANVAS_EDGE.createdAt}
                </p>
              </div>

              <div className="mt-5 border-t border-line pt-4">
                <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                  connections
                </p>
                <ul className="mt-2 space-y-2">
                  {INSPECTOR_CONNECTIONS.map((connection) => (
                    <li
                      key={connection.other}
                      className="flex items-center justify-between gap-2 text-xs"
                    >
                      <span className="truncate text-muted">
                        {connection.relation}{" "}
                        <span className="text-foreground">{connection.other}</span>
                      </span>
                      <Badge
                        tone={
                          connection.confidence === "EXTRACTED" ? "accent" : "neutral"
                        }
                      >
                        {connection.confidence === "EXTRACTED" ? "ext" : "inf"}
                      </Badge>
                    </li>
                  ))}
                </ul>
              </div>
            </aside>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-1 gap-6 border-t border-line pt-8 sm:grid-cols-3">
          {FEATURES.map((feature) => (
            <div key={feature.title} className="flex gap-3">
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
      </div>
    </section>
  );
}
