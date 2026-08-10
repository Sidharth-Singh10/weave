"use client";

import { useMemo } from "react";
import type { Edge } from "@xyflow/react";
import {
  type GhostNode,
  type KnowledgeEdge,
  type KnowledgeNode,
  type WeaveFlowNode,
  type XYPosition,
} from "./graph-types";
import { useGraphStore } from "./store";

export interface RenderState {
  renderNodes: WeaveFlowNode[];
  renderEdges: Edge[];
}

/**
 * Identity projection from knowledge + layout metadata to ReactFlow render
 * objects. This is where Iteration 2 view filtering/grouping will plug in.
 * For now every knowledge node/edge is visible.
 */
export function projectToRender(
  knowledgeNodes: KnowledgeNode[],
  knowledgeEdges: KnowledgeEdge[],
  positions: Record<string, XYPosition>,
  freshIds: string[],
  ghostNode: GhostNode | null
): RenderState {
  const freshSet = new Set(freshIds);

  const renderNodes: WeaveFlowNode[] = knowledgeNodes.map((n) => ({
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

  const renderEdges: Edge[] = knowledgeEdges.map((e) => ({
    id: e.id,
    source: e.source,
    target: e.target,
    label: e.relation,
    type: "default",
  }));

  return { renderNodes, renderEdges };
}

/** Subscribe to the knowledge graph and project it to ReactFlow objects. */
export function useRenderGraph(): RenderState {
  const knowledgeNodes = useGraphStore((s) => s.knowledgeNodes);
  const knowledgeEdges = useGraphStore((s) => s.knowledgeEdges);
  const positions = useGraphStore((s) => s.positions);
  const freshIds = useGraphStore((s) => s.freshIds);
  const ghostNode = useGraphStore((s) => s.ghostNode);

  return useMemo(
    () =>
      projectToRender(
        knowledgeNodes,
        knowledgeEdges,
        positions,
        freshIds,
        ghostNode
      ),
    [knowledgeNodes, knowledgeEdges, positions, freshIds, ghostNode]
  );
}
