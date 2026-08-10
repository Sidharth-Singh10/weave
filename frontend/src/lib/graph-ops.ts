import type { Edge } from "@xyflow/react";
import type { GraphDelta, WeaveFlowNode } from "./graph-types";

export interface ApplyDeltaResult {
  nodes: WeaveFlowNode[];
  edges: Edge[];
}

export function slug(label: string) {
  return label
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}

/** Find a position for a new node: radially around its primary parent if any,
 * otherwise offset from the graph's centroid. */
function placeNode(
  parent: WeaveFlowNode | undefined,
  siblings: number,
  index: number
): { x: number; y: number } {
  const angle = (Math.PI * 2 * (index + siblings)) / Math.max(siblings + 1, 3);
  const radius = 220;
  if (parent) {
    return {
      x: parent.position.x + radius * Math.cos(angle),
      y: parent.position.y + radius * Math.sin(angle),
    };
  }
  return { x: radius * Math.cos(angle), y: radius * Math.sin(angle) };
}

/**
 * Central graph mutation. Merges a validated graph delta into the current
 * graph and returns the next graph state. Pure: no store access, no side
 * effects. Node identity prefers stable ids, then label matching, then fresh
 * creation.
 */
export function applyGraphDelta(
  nodes: WeaveFlowNode[],
  edges: Edge[],
  delta: GraphDelta
): ApplyDeltaResult {
  const byId = new Map(nodes.map((n) => [n.id, n] as const));
  const byLabel = new Map(
    nodes.map((n) => [n.data.label.toLowerCase(), n] as const)
  );

  const newNodes: WeaveFlowNode[] = [];
  const newEdges: Edge[] = [];

  // Generate a stable `node-{slug}` id, deduping against existing ids.
  const usedIds = new Set(nodes.map((n) => n.id));
  const freshNodeId = (label: string): string => {
    const base = `node-${slug(label) || "node"}`;
    if (!usedIds.has(base)) {
      usedIds.add(base);
      return base;
    }
    let i = 2;
    while (usedIds.has(`${base}-${i}`)) i += 1;
    const id = `${base}-${i}`;
    usedIds.add(id);
    return id;
  };

  // Resolve a node to an id: prefer the backend-provided id, then the
  // existing label, then create fresh. Tracks nodes added in this pass.
  const resolveId = (
    id: string | undefined,
    label: string,
    kind: string
  ): { id: string; isNew: boolean } => {
    if (id) {
      const existing = byId.get(id);
      if (existing) return { id: existing.id, isNew: false };
    }
    const key = label.toLowerCase();
    const existing = byLabel.get(key);
    if (existing) return { id: existing.id, isNew: false };
    const nodeId = id ?? freshNodeId(label);
    const node: WeaveFlowNode = {
      id: nodeId,
      type: "weave",
      position: { x: 0, y: 0 },
      data: { label, kind, fresh: true },
    };
    byId.set(nodeId, node);
    byLabel.set(key, node);
    newNodes.push(node);
    return { id: nodeId, isNew: true };
  };

  // Create nodes first so every edge endpoint exists.
  for (const n of delta.nodes) resolveId(n.id, n.label, n.kind);

  // Count how many new nodes attach to each parent for radial layout.
  const parentChildCount = new Map<string, number>();

  for (const e of delta.edges) {
    const src = resolveId(e.source_id, e.source_label, "concept");
    const tgt = resolveId(e.target_id, e.target_label, "concept");
    const edgeId = `${src.id}->${tgt.id}:${slug(e.relation)}`;
    const dup = [...edges, ...newEdges].some((x) => x.id === edgeId);
    if (!dup && src.id !== tgt.id) {
      newEdges.push({
        id: edgeId,
        source: src.id,
        target: tgt.id,
        label: e.relation,
        type: "default",
      });
      parentChildCount.set(src.id, (parentChildCount.get(src.id) ?? 0) + 1);
    }
  }

  // Position new nodes around their primary parent (first edge source).
  const parentOf = new Map<string, string>();
  for (const e of delta.edges) {
    const key = e.target_label.toLowerCase();
    if (!parentOf.has(key)) {
      const srcNode = byLabel.get(e.source_label.toLowerCase());
      if (srcNode) parentOf.set(key, srcNode.id);
    }
  }

  const childIndex = new Map<string, number>();
  for (const node of newNodes) {
    const parentId = parentOf.get(node.data.label.toLowerCase());
    const parent = parentId
      ? [...nodes, ...newNodes].find((n) => n.id === parentId)
      : undefined;
    const siblings = parentId ? (parentChildCount.get(parentId) ?? 1) - 1 : 0;
    const idx = childIndex.get(parentId ?? "") ?? 0;
    childIndex.set(parentId ?? "", idx + 1);
    node.position = placeNode(parent, siblings, idx);
  }

  return {
    nodes: [...nodes, ...newNodes],
    edges: [...edges, ...newEdges],
  };
}
