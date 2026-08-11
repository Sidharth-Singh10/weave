"use client";

import { useMemo } from "react";
import type { Edge } from "@xyflow/react";
import {
  type GhostNode,
  type KnowledgeEdge,
  type KnowledgeNode,
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
 * The view engine. Transforms the knowledge graph + a view config into the
 * set of knowledge entities that should currently be displayed. Iteration 2
 * features (importance, semantic zoom, communities, flavors) plug in here.
 *
 * Default view: identity projection — everything is visible.
 */
export function createGraphView(
  knowledgeNodes: KnowledgeNode[],
  knowledgeEdges: KnowledgeEdge[],
  config: ViewConfig
): GraphView {
  switch (config.type) {
    case "default":
    default:
      return {
        visibleNodes: knowledgeNodes,
        visibleEdges: knowledgeEdges,
        importance: {},
      };
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
