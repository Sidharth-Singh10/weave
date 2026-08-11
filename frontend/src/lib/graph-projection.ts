"use client";

import { useMemo } from "react";
import type { Edge } from "@xyflow/react";
import {
  type GhostNode,
  type KnowledgeEdge,
  type KnowledgeNode,
  type SemanticZoomLevel,
  type ViewConfig,
  type WeaveFlowNode,
  type XYPosition,
} from "./graph-types";
import { detectCommunities } from "./communities";
import { useGraphStore } from "./store";

/** Muted hues for community-colored edges, on the dark canvas. */
export const COMMUNITY_COLORS = [
  "#8b5cf6", // violet
  "#14b8a6", // teal
  "#f59e0b", // amber
  "#ec4899", // pink
  "#3b82f6", // blue
  "#22c55e", // green
];

/** Topic colors keyed by node kind, used by the Topic view projection. */
export const TOPIC_COLORS: Record<string, string> = {
  person: "#8b5cf6",
  place: "#14b8a6",
  org: "#f59e0b",
  event: "#ec4899",
  object: "#3b82f6",
  concept: "#22c55e",
};

function topicColor(kind: string): string {
  return TOPIC_COLORS[kind] ?? "#a1a1aa";
}

function communityColor(communityId: number): string {
  return COMMUNITY_COLORS[communityId % COMMUNITY_COLORS.length];
}

/**
 * The result of projecting knowledge through a view config: which knowledge
 * entities are visible, plus derived display metadata (importance, groups...).
 */
export interface GraphView {
  visibleNodes: KnowledgeNode[];
  visibleEdges: KnowledgeEdge[];
  importance: Record<string, number>;
  /** nodeId -> community id, from the full knowledge graph. */
  communities: Record<string, number>;
  /** nodeId -> degree centrality, from the full knowledge graph. */
  degree: Record<string, number>;
  /** nodeId -> topic color, populated by the Topic view projection. */
  topics?: Record<string, string>;
}

export interface RenderState {
  renderNodes: WeaveFlowNode[];
  renderEdges: Edge[];
}

/**
 * Deterministic importance: normalized degree centrality.
 * A hub node (many connections) scores 1.0; an isolated node scores 0.
 * Pure graph metric — visualization metadata, never a knowledge mutation.
 */
export function computeImportance(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[]
): Record<string, number> {
  const degree = computeDegree(nodes, edges);
  const max = Math.max(...Object.values(degree), 1);
  const result: Record<string, number> = {};
  for (const [id, d] of Object.entries(degree)) {
    result[id] = max > 0 ? d / max : 0.5;
  }
  return result;
}

/** Raw degree centrality: number of incident edges per node. */
export function computeDegree(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[]
): Record<string, number> {
  const degree: Record<string, number> = {};
  for (const n of nodes) degree[n.id] = 0;
  for (const e of edges) {
    if (e.source in degree) degree[e.source] += 1;
    if (e.target in degree) degree[e.target] += 1;
  }
  return degree;
}

/**
 * Focused projection: the selected node plus its neighborhood within
 * `depth` hops, and the edges among those nodes. Reversible — knowledge is
 * never mutated. Higher depth = deeper adaptive granularity.
 */
function focusedView(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[],
  selectedId: string,
  depth: number,
  importance: Record<string, number>,
  communities: Record<string, number>,
  degree: Record<string, number>
): GraphView {
  const selected = nodes.find((n) => n.id === selectedId);
  if (!selected) {
    return {
      visibleNodes: nodes,
      visibleEdges: edges,
      importance,
      communities,
      degree,
    };
  }

  const visibleIds = new Set<string>([selectedId]);
  const adj = new Map<string, string[]>();
  for (const e of edges) {
    const s = adj.get(e.source) ?? [];
    s.push(e.target);
    adj.set(e.source, s);
    const t = adj.get(e.target) ?? [];
    t.push(e.source);
    adj.set(e.target, t);
  }

  const queue: string[] = [selectedId];
  const dist = new Map<string, number>([[selectedId, 0]]);
  while (queue.length > 0) {
    const cur = queue.shift() as string;
    const d = dist.get(cur) ?? 0;
    if (d >= depth) continue;
    for (const nb of adj.get(cur) ?? []) {
      if (!dist.has(nb)) {
        dist.set(nb, d + 1);
        visibleIds.add(nb);
        queue.push(nb);
      }
    }
  }

  return {
    visibleNodes: nodes.filter((n) => visibleIds.has(n.id)),
    visibleEdges: edges.filter(
      (e) => visibleIds.has(e.source) && visibleIds.has(e.target)
    ),
    importance,
    communities,
    degree,
  };
}

/**
 * Semantic zoom: higher abstraction levels hide lower-importance nodes.
 * overview shows only prominent nodes, category shows notable ones,
 * entity/detail show everything. Always keeps at least a small set visible
 * so zoomed-out views are never empty. Deterministic — no LLM involvement.
 */
function filterByImportance(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[],
  importance: Record<string, number>,
  communities: Record<string, number>,
  degree: Record<string, number>,
  level: SemanticZoomLevel
): GraphView {
  if (level === "entity" || level === "detail") {
    return {
      visibleNodes: nodes,
      visibleEdges: edges,
      importance,
      communities,
      degree,
    };
  }
  const threshold = level === "overview" ? 0.5 : 0.25;
  const floor = 3;

  const ranked = [...nodes].sort(
    (a, b) => (importance[b.id] ?? 0) - (importance[a.id] ?? 0)
  );
  const visible = new Set<string>();
  for (const n of ranked) {
    if ((importance[n.id] ?? 0) >= threshold || visible.size < floor) {
      visible.add(n.id);
    }
  }

  return {
    visibleNodes: nodes.filter((n) => visible.has(n.id)),
    visibleEdges: edges.filter(
      (e) => visible.has(e.source) && visible.has(e.target)
    ),
    importance,
    communities,
    degree,
  };
}

/**
 * The view engine. Transforms the knowledge graph + a view config into the
 * set of knowledge entities that should currently be displayed. Iteration 2
 * features (importance, semantic zoom, communities, flavors) plug in here.
 *
 * Default view: identity projection; selecting a node focuses its
 * neighborhood (which disables zoom filtering), otherwise semantic zoom
 * controls information density. Importance is always derived from the full
 * graph so hubs stay visually prominent.
 */
export function createGraphView(
  knowledgeNodes: KnowledgeNode[],
  knowledgeEdges: KnowledgeEdge[],
  config: ViewConfig
): GraphView {
  const importance = computeImportance(knowledgeNodes, knowledgeEdges);
  const communities = Object.fromEntries(
    detectCommunities(knowledgeNodes, knowledgeEdges)
  );
  const degree = computeDegree(knowledgeNodes, knowledgeEdges);

  let view: GraphView;
  switch (config.type) {
    case "topic":
      view = config.selectedNodeId
        ? focusedView(
            knowledgeNodes,
            knowledgeEdges,
            config.selectedNodeId,
            config.focusDepth ?? 1,
            importance,
            communities,
            degree
          )
        : filterByImportance(
            knowledgeNodes,
            knowledgeEdges,
            importance,
            communities,
            degree,
            config.semanticZoom
          );
      // Topic flavor: same knowledge, kind-tinted node accents + legend.
      const topics: Record<string, string> = {};
      for (const n of knowledgeNodes) topics[n.id] = topicColor(n.kind);
      view = { ...view, topics };
      break;
    case "default":
    default:
      view = config.selectedNodeId
        ? focusedView(
            knowledgeNodes,
            knowledgeEdges,
            config.selectedNodeId,
            config.focusDepth ?? 1,
            importance,
            communities,
            degree
          )
        : filterByImportance(
            knowledgeNodes,
            knowledgeEdges,
            importance,
            communities,
            degree,
            config.semanticZoom
          );
      break;
  }
  return view;
}

/**
 * Project a GraphView + layout metadata into ReactFlow render objects.
 * Every visible knowledge node/edge becomes a render node/edge.
 */
export function projectToRender(
  view: GraphView,
  positions: Record<string, XYPosition>,
  freshIds: string[],
  ghostNode: GhostNode | null
): RenderState {
  const freshSet = new Set(freshIds);

  const renderNodes: WeaveFlowNode[] = view.visibleNodes.map((n) => ({
    id: n.id,
    type: "weave",
    position: positions[n.id] ?? { x: 0, y: 0 },
    data: {
      label: n.label,
      kind: n.kind,
      fresh: freshSet.has(n.id),
      importance: view.importance[n.id],
      degree: view.degree[n.id],
      topicColor: view.topics?.[n.id],
    },
  }));

  if (ghostNode) {
    renderNodes.push({
      id: ghostNode.id,
      type: "weave",
      position: ghostNode.position,
      data: { label: "Weaving...", kind: "", fresh: false, ghost: true },
      selectable: false,
      draggable: false,
    });
  }

  const renderEdges: Edge[] = view.visibleEdges.map((e) => {
    const a = view.communities[e.source];
    const b = view.communities[e.target];
    const inCommunity = a !== undefined && a === b;
    const style = inCommunity
      ? {
          stroke: communityColor(a as number),
          strokeOpacity: 0.5,
          strokeWidth: 1.5,
        }
      : undefined;
    return {
      id: e.id,
      source: e.source,
      target: e.target,
      label: e.relation,
      type: "default",
      style,
    };
  });

  return { renderNodes, renderEdges };
}

/** Subscribe to the knowledge graph, project it through the view, and map to
 * ReactFlow objects. */
export function useRenderGraph(): RenderState {
  const knowledgeNodes = useGraphStore((s) => s.knowledgeNodes);
  const knowledgeEdges = useGraphStore((s) => s.knowledgeEdges);
  const positions = useGraphStore((s) => s.positions);
  const freshIds = useGraphStore((s) => s.freshIds);
  const ghostNode = useGraphStore((s) => s.ghostNode);
  const viewConfig = useGraphStore((s) => s.viewConfig);

  const view = useMemo(
    () => createGraphView(knowledgeNodes, knowledgeEdges, viewConfig),
    [knowledgeNodes, knowledgeEdges, viewConfig]
  );

  return useMemo(
    () => projectToRender(view, positions, freshIds, ghostNode),
    [view, positions, freshIds, ghostNode]
  );
}
