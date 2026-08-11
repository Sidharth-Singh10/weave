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
import { useGraphStore } from "./store";

/**
 * The result of projecting knowledge through a view config: which knowledge
 * entities are visible, plus derived display metadata (importance, groups...).
 */
export interface GraphView {
  visibleNodes: KnowledgeNode[];
  visibleEdges: KnowledgeEdge[];
  importance: Record<string, number>;
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
  const degree = new Map<string, number>();
  for (const n of nodes) degree.set(n.id, 0);
  for (const e of edges) {
    if (degree.has(e.source)) degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
    if (degree.has(e.target)) degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
  }
  const max = Math.max(...degree.values(), 1);
  const result: Record<string, number> = {};
  for (const [id, d] of degree) {
    result[id] = max > 0 ? d / max : 0.5;
  }
  return result;
}

/**
 * Focused projection: the selected node plus its direct (1-hop) neighborhood,
 * and the edges among those nodes. Reversible — knowledge is never mutated.
 */
function focusedView(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[],
  selectedId: string,
  importance: Record<string, number>
): GraphView {
  const selected = nodes.find((n) => n.id === selectedId);
  if (!selected) {
    return { visibleNodes: nodes, visibleEdges: edges, importance };
  }

  const visibleIds = new Set<string>([selectedId]);
  for (const e of edges) {
    if (e.source === selectedId) visibleIds.add(e.target);
    if (e.target === selectedId) visibleIds.add(e.source);
  }

  return {
    visibleNodes: nodes.filter((n) => visibleIds.has(n.id)),
    visibleEdges: edges.filter(
      (e) => visibleIds.has(e.source) && visibleIds.has(e.target)
    ),
    importance,
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
  level: SemanticZoomLevel
): GraphView {
  if (level === "entity" || level === "detail") {
    return { visibleNodes: nodes, visibleEdges: edges, importance };
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

  switch (config.type) {
    case "default":
    default:
      if (config.selectedNodeId) {
        return focusedView(
          knowledgeNodes,
          knowledgeEdges,
          config.selectedNodeId,
          importance
        );
      }
      return filterByImportance(
        knowledgeNodes,
        knowledgeEdges,
        importance,
        config.semanticZoom
      );
  }
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

  const renderEdges: Edge[] = view.visibleEdges.map((e) => ({
    id: e.id,
    source: e.source,
    target: e.target,
    label: e.relation,
    type: "default",
  }));

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
