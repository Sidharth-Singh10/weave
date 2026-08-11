import type { KnowledgeEdge, KnowledgeNode, XYPosition } from "./graph-types";
import { detectCommunities } from "./communities";
import { computeImportance } from "./graph-projection";

/** Semantic constraints the layout engine derives from the knowledge graph. */
export interface LayoutContext {
  /** nodeId -> community id. */
  community: Record<string, number>;
  /** community id -> dominant sector angle (radians) for clustering. */
  communitySector: Record<number, number>;
  /** nodeId -> importance (0-1). */
  importance: Record<string, number>;
}

/** Derive deterministic layout constraints: communities + importance. */
export function computeLayoutContext(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[]
): LayoutContext {
  const community = Object.fromEntries(detectCommunities(nodes, edges));
  const importance = computeImportance(nodes, edges);

  const communityIds = Array.from(
    new Set(Object.values(community))
  ).sort((a, b) => a - b);
  const communitySector: Record<number, number> = {};
  communityIds.forEach((cid, i) => {
    communitySector[cid] = (Math.PI * 2 * i) / Math.max(communityIds.length, 1);
  });

  return { community, communitySector, importance };
}

/**
 * Deterministic community-ring layout. Positions are visual metadata only —
 * knowledge is never mutated.
 *
 * - communities get anchors spread around the graph centroid,
 * - members of a community sit on a small ring around their anchor,
 * - importance orders members so hubs land nearest the anchor (most central),
 * - existing positions only inform the centroid, so the layout stays near
 *   where the user was working.
 */
export function relayout(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[],
  existingPositions: Record<string, XYPosition>
): Record<string, XYPosition> {
  const positions: Record<string, XYPosition> = {};
  if (nodes.length === 0) return positions;

  const { community, importance } = computeLayoutContext(nodes, edges);

  const groups = new Map<number, string[]>();
  for (const n of nodes) {
    const c = community[n.id] ?? 0;
    const arr = groups.get(c) ?? [];
    arr.push(n.id);
    groups.set(c, arr);
  }
  const ids = Array.from(groups.keys()).sort((a, b) => a - b);
  const nGroups = Math.max(ids.length, 1);

  const entries = Object.values(existingPositions);
  let cx = 0;
  let cy = 0;
  if (entries.length > 0) {
    cx = entries.reduce((s, p) => s + p.x, 0) / entries.length;
    cy = entries.reduce((s, p) => s + p.y, 0) / entries.length;
  }

  ids.forEach((cid, gi) => {
    const members = groups.get(cid)!;
    // Single community anchors near the center; several spread on a ring.
    const anchorAngle = nGroups === 1 ? 0 : (Math.PI * 2 * gi) / nGroups;
    const anchorR =
      nGroups === 1 ? 0 : 150 + 60 * Math.log2(Math.max(nGroups, 2));
    const ax = cx + anchorR * Math.cos(anchorAngle);
    const ay = cy + anchorR * Math.sin(anchorAngle);

    // Most important members closest to the community anchor.
    const sorted = [...members].sort(
      (a, b) => (importance[b] ?? 0) - (importance[a] ?? 0)
    );
    const n = sorted.length;
    sorted.forEach((id, i) => {
      const innerR = n === 1 ? 0 : 40 + 110 * (i / (n - 1));
      const innerAngle = (Math.PI * 2 * i) / n + cid * 0.618;
      positions[id] = {
        x: ax + innerR * Math.cos(innerAngle),
        y: ay + innerR * Math.sin(innerAngle),
      };
    });
  });

  return positions;
}
