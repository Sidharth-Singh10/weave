"use client";

import { useId } from "react";
import { motion, useReducedMotion } from "motion/react";
import {
  COMMUNITY_COLORS,
  type Confidence,
  type GraphEdgeSpec,
  type GraphLevel,
  type GraphNodeSpec,
} from "@/lib/landing-content";
import { cn } from "@/lib/cn";

const WIDTH = 720;
const HEIGHT = 440;

const CONFIDENCE_STROKE: Record<Confidence, string> = {
  EXTRACTED: "var(--accent)",
  INFERRED: "var(--faint)",
  AMBIGUOUS: "var(--warning)",
};

function nodeBox(node: GraphNodeSpec) {
  const importance = Math.max(0, Math.min(1, node.importance ?? 0.5));
  const width = Math.max(84, node.label.length * 6.3 + 24);
  const height = 30 + importance * 6;
  return { width, height, importance };
}

function edgePath(
  from: GraphNodeSpec,
  to: GraphNodeSpec,
  index: number
): { d: string; labelX: number; labelY: number } {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.max(1, Math.hypot(dx, dy));
  const bend = (index % 2 === 0 ? 1 : -1) * Math.min(22, length * 0.09);
  const controlX = (from.x + to.x) / 2 + (-dy / length) * bend;
  const controlY = (from.y + to.y) / 2 + (dx / length) * bend;
  return {
    d: `M ${from.x} ${from.y} Q ${controlX} ${controlY} ${to.x} ${to.y}`,
    labelX: 0.25 * from.x + 0.5 * controlX + 0.25 * to.x,
    labelY: 0.25 * from.y + 0.5 * controlY + 0.25 * to.y - 5,
  };
}

export function GraphStage({
  nodes,
  edges,
  level,
  communities,
  activeIds,
  className,
  label,
}: {
  nodes: GraphNodeSpec[];
  edges: GraphEdgeSpec[];
  level: GraphLevel;
  communities?: Record<number, string>;
  activeIds?: string[];
  className?: string;
  label: string;
}) {
  const reduce = useReducedMotion();
  const uid = useId().replace(/:/g, "");
  const titleId = `${uid}-title`;
  const descId = `${uid}-desc`;
  const gridId = `${uid}-grid`;

  const byId = new Map(nodes.map((node) => [node.id, node] as const));
  const visibleNodes = nodes.filter((node) => (node.minLevel ?? 0) <= level);
  const visibleIds = new Set(visibleNodes.map((node) => node.id));
  const visibleEdges = edges.filter(
    (edge) =>
      (edge.minLevel ?? 0) <= level &&
      byId.has(edge.from) &&
      byId.has(edge.to) &&
      visibleIds.has(edge.from) &&
      visibleIds.has(edge.to)
  );
  const active = activeIds && activeIds.length > 0 ? new Set(activeIds) : null;

  // Fit the drawing to the visible nodes instead of using the whole fixed
  // canvas, so a three-node example does not float in empty space.
  let viewX = 0;
  let viewY = 0;
  let viewW = WIDTH;
  let viewH = HEIGHT;
  if (visibleNodes.length > 0) {
    const boxes = visibleNodes.map((node) => ({
      node,
      box: nodeBox(node),
    }));
    const minX = Math.min(...boxes.map((b) => b.node.x - b.box.width / 2));
    const maxX = Math.max(...boxes.map((b) => b.node.x + b.box.width / 2));
    const minY = Math.min(...boxes.map((b) => b.node.y - b.box.height / 2));
    const maxY = Math.max(...boxes.map((b) => b.node.y + b.box.height / 2));
    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;
    viewW = Math.max(maxX - minX + 104, 360);
    viewH = Math.max(maxY - minY + 104, 280);
    viewX = centerX - viewW / 2;
    viewY = centerY - viewH / 2;
  }

  const hulls: {
    id: number;
    x: number;
    y: number;
    width: number;
    height: number;
    color: string;
    label: string;
  }[] = [];
  if (communities && level >= 1) {
    const grouped = new Map<number, GraphNodeSpec[]>();
    for (const node of visibleNodes) {
      if (node.community === undefined) continue;
      const list = grouped.get(node.community) ?? [];
      list.push(node);
      grouped.set(node.community, list);
    }
    for (const [community, members] of grouped) {
      if (members.length < 2) continue;
      const boxes = members.map((member) => ({
        node: member,
        box: nodeBox(member),
      }));
      const minX = Math.min(...boxes.map((b) => b.node.x - b.box.width / 2));
      const maxX = Math.max(...boxes.map((b) => b.node.x + b.box.width / 2));
      const minY = Math.min(...boxes.map((b) => b.node.y - b.box.height / 2));
      const maxY = Math.max(...boxes.map((b) => b.node.y + b.box.height / 2));
      hulls.push({
        id: community,
        x: minX - 26,
        y: minY - 34,
        width: maxX - minX + 52,
        height: maxY - minY + 58,
        color: COMMUNITY_COLORS[community % COMMUNITY_COLORS.length],
        label: communities[community] ?? `Group ${community + 1}`,
      });
    }
  }

  return (
    <svg
      viewBox={`${viewX} ${viewY} ${viewW} ${viewH}`}
      role="img"
      aria-labelledby={`${titleId} ${descId}`}
      className={cn("h-full w-full", className)}
      preserveAspectRatio="xMidYMid meet"
    >
      <title id={titleId}>{label}</title>
      <desc id={descId}>
        A knowledge graph illustration. Nodes are concepts, edges are relations.
        {level >= 3
          ? " Relation labels and grounding classes are shown."
          : ""}
      </desc>

      <defs>
        <pattern
          id={gridId}
          width="26"
          height="26"
          patternUnits="userSpaceOnUse"
        >
          <circle cx="1" cy="1" r="1" fill="var(--line)" />
        </pattern>
      </defs>
      <rect
        x={viewX}
        y={viewY}
        width={viewW}
        height={viewH}
        fill={`url(#${gridId})`}
        opacity="0.5"
      />

      <motion.g key={`level-${level}`}>
        {hulls.map((hull) => (
          <g key={hull.id}>
            <rect
              x={hull.x}
              y={hull.y}
              width={hull.width}
              height={hull.height}
              rx="26"
              fill={hull.color}
              fillOpacity="0.045"
              stroke={hull.color}
              strokeOpacity="0.22"
              strokeDasharray="4 6"
            />
            <text
              x={hull.x + 16}
              y={hull.y + 21}
              fontFamily="var(--font-geist-mono)"
              fontSize="9"
              letterSpacing="1.4"
              fill={hull.color}
              fillOpacity="0.8"
            >
              {hull.label.toUpperCase()}
            </text>
          </g>
        ))}

        {visibleEdges.map((edge, index) => {
          const from = byId.get(edge.from);
          const to = byId.get(edge.to);
          if (!from || !to) return null;
          const path = edgePath(from, to, index);
          const confidence = edge.confidence ?? "EXTRACTED";
          const touched = active
            ? active.has(edge.from) || active.has(edge.to)
            : true;
          const dimmed = active ? !touched : false;
          const stroke = CONFIDENCE_STROKE[confidence];
          const dashed =
            confidence === "INFERRED"
              ? "5 5"
              : confidence === "AMBIGUOUS"
                ? "3 4"
                : undefined;

          return (
            <g key={edge.id} opacity={dimmed ? 0.22 : 1}>
              <motion.path
                d={path.d}
                fill="none"
                stroke={stroke}
                strokeWidth={confidence === "EXTRACTED" ? 1.5 : 1.3}
                strokeDasharray={dashed}
                strokeOpacity={confidence === "EXTRACTED" ? 0.62 : 0.72}
                initial={reduce ? false : { pathLength: 0, opacity: 0 }}
                animate={{ pathLength: 1, opacity: 1 }}
                transition={{
                  duration: 0.55,
                  delay: 0.08 + index * 0.035,
                  ease: "easeOut",
                }}
              />
              {level >= 3 && edge.relation && (
                <text
                  x={path.labelX}
                  y={path.labelY}
                  textAnchor="middle"
                  fontFamily="var(--font-geist-mono)"
                  fontSize="8.5"
                  fill="var(--faint)"
                  stroke="var(--background)"
                  strokeWidth="3"
                  paintOrder="stroke"
                >
                  {edge.relation}
                </text>
              )}
            </g>
          );
        })}

        {visibleNodes.map((node, index) => {
          const { width, height, importance } = nodeBox(node);
          const isHub = importance >= 0.8;
          const communityColor =
            node.community !== undefined
              ? COMMUNITY_COLORS[node.community % COMMUNITY_COLORS.length]
              : "var(--line-bright)";
          const highlighted = active ? active.has(node.id) : true;
          const dimmed = active ? !highlighted : false;

          return (
            <motion.g
              key={node.id}
              opacity={dimmed ? 0.28 : 1}
              initial={reduce ? false : { opacity: 0 }}
              animate={{ opacity: dimmed ? 0.28 : 1 }}
              transition={{ duration: 0.3, delay: 0.05 + index * 0.03 }}
            >
              {isHub && (
                <rect
                  x={node.x - width / 2 - 4}
                  y={node.y - height / 2 - 4}
                  width={width + 8}
                  height={height + 8}
                  rx="15"
                  fill="none"
                  stroke="var(--accent)"
                  strokeOpacity="0.18"
                />
              )}
              <rect
                x={node.x - width / 2}
                y={node.y - height / 2}
                width={width}
                height={height}
                rx="12"
                fill="var(--surface-2)"
                stroke={isHub ? "var(--accent)" : communityColor}
                strokeOpacity={isHub ? 0.9 : 0.45}
                strokeWidth={isHub ? 1.4 : 1}
              />
              <text
                x={node.x}
                y={node.y + (level >= 3 && node.kind ? -3 : 1)}
                textAnchor="middle"
                dominantBaseline="middle"
                fontFamily="var(--font-geist-sans)"
                fontSize={isHub ? 12.5 : 11.5}
                fontWeight={isHub ? 600 : 500}
                fill="var(--foreground)"
              >
                {node.label}
              </text>
              {level >= 3 && node.kind && (
                <text
                  x={node.x}
                  y={node.y + 10}
                  textAnchor="middle"
                  fontFamily="var(--font-geist-mono)"
                  fontSize="8"
                  letterSpacing="1.1"
                  fill="var(--faint)"
                >
                  {node.kind.toUpperCase()}
                </text>
              )}
              {(node.degree ?? 0) > 3 && level >= 2 && (
                <g>
                  <circle
                    cx={node.x + width / 2 - 2}
                    cy={node.y - height / 2 + 2}
                    r="8"
                    fill="var(--accent)"
                  />
                  <text
                    x={node.x + width / 2 - 2}
                    y={node.y - height / 2 + 5}
                    textAnchor="middle"
                    fontFamily="var(--font-geist-mono)"
                    fontSize="8"
                    fontWeight="700"
                    fill="var(--accent-ink)"
                  >
                    {node.degree}
                  </text>
                </g>
              )}
            </motion.g>
          );
        })}
      </motion.g>
    </svg>
  );
}
