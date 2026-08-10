import type { KnowledgeEdge, KnowledgeNode, XYPosition } from "./graph-types";
import type { GraphDelta } from "./graph-types";

export interface ApplyDeltaResult {
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  /** ids of nodes created in this pass (used for the "fresh" pulse). */
  newIds: string[];
  /** positions assigned to the new nodes. */
  positions: Record<string, XYPosition>;
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
  parentPos: XYPosition | undefined,
  siblings: number,
  index: number
): XYPosition {
  const angle = (Math.PI * 2 * (index + siblings)) / Math.max(siblings + 1, 3);
  const radius = 220;
  if (parentPos) {
    return {
      x: parentPos.x + radius * Math.cos(angle),
      y: parentPos.y + radius * Math.sin(angle),
    };
  }
  return { x: radius * Math.cos(angle), y: radius * Math.sin(angle) };
}

/**
 * Central knowledge mutation. Merges a validated graph delta into the current
 * knowledge graph and returns the next knowledge state plus layout positions
 * for the new nodes. Pure: no store access, no side effects. Node identity
 * prefers stable ids, then label matching, then fresh creation.
 */
export function applyGraphDelta(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[],
  delta: GraphDelta,
  existingPositions: Record<string, XYPosition>
): ApplyDeltaResult {
  const byId = new Map(nodes.map((n) => [n.id, n] as const));
  const byLabel = new Map(
    nodes.map((n) => [n.label.toLowerCase(), n] as const)
  );

  const newNodes: KnowledgeNode[] = [];
  const newEdges: KnowledgeEdge[] = [];
  const newIds: string[] = [];
  const positions: Record<string, XYPosition> = {};

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
  // existing label, then create fresh. Registers new nodes for edge creation.
  const resolveId = (
    id: string | undefined,
    label: string,
    kind: string
  ): string => {
    if (id) {
      const existing = byId.get(id);
      if (existing) return existing.id;
    }
    const key = label.toLowerCase();
    const existing = byLabel.get(key);
    if (existing) return existing.id;
    const nodeId = id ?? freshNodeId(label);
    const node: KnowledgeNode = { id: nodeId, label, kind };
    byId.set(nodeId, node);
    byLabel.set(key, node);
    newNodes.push(node);
    newIds.push(nodeId);
    return nodeId;
  };

  // Create nodes first so every edge endpoint exists.
  for (const n of delta.nodes) resolveId(n.id, n.label, n.kind);

  // Count how many new nodes attach to each parent for radial layout.
  const parentChildCount = new Map<string, number>();

  for (const e of delta.edges) {
    const srcId = resolveId(e.source_id, e.source_label, "concept");
    const tgtId = resolveId(e.target_id, e.target_label, "concept");
    const edgeId = `${srcId}->${tgtId}:${slug(e.relation)}`;
    const dup = [...edges, ...newEdges].some((x) => x.id === edgeId);
    if (!dup && srcId !== tgtId) {
      newEdges.push({
        id: edgeId,
        source: srcId,
        target: tgtId,
        relation: e.relation,
      });
      parentChildCount.set(srcId, (parentChildCount.get(srcId) ?? 0) + 1);
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
    const parentId = parentOf.get(node.label.toLowerCase());
    const parentPos = parentId
      ? positions[parentId] ?? existingPositions[parentId]
      : undefined;
    const siblings = parentId ? (parentChildCount.get(parentId) ?? 1) - 1 : 0;
    const idx = childIndex.get(parentId ?? "") ?? 0;
    childIndex.set(parentId ?? "", idx + 1);
    positions[node.id] = placeNode(parentPos, siblings, idx);
  }

  return {
    nodes: [...nodes, ...newNodes],
    edges: [...edges, ...newEdges],
    newIds,
    positions,
  };
}
