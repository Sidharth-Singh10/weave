import type { KnowledgeEdge, KnowledgeNode } from "./graph-types";

/** nodeId -> community id (0-based, assigned in first-seen order). */
export type CommunityAssignments = Map<string, number>;

/**
 * Label propagation community detection.
 *
 * Every node starts with its own label; on each pass a node adopts the label
 * most common among its neighbors (smallest label breaks ties for
 * determinism). Converges in a few passes for small graphs. Pure graph
 * algorithm — no LLM, no mutation of the knowledge graph.
 */
export function detectCommunities(
  nodes: KnowledgeNode[],
  edges: KnowledgeEdge[]
): CommunityAssignments {
  const adj = new Map<string, Set<string>>();
  for (const n of nodes) adj.set(n.id, new Set());
  for (const e of edges) {
    const s = adj.get(e.source);
    const t = adj.get(e.target);
    if (!s || !t) continue;
    s.add(e.target);
    t.add(e.source);
  }

  // Initial label = node index; each node then propagates neighbor labels.
  const labels = new Map<string, number>();
  nodes.forEach((n, i) => labels.set(n.id, i));

  const propagate = (): boolean => {
    let changed = false;
    for (const n of nodes) {
      const neighbors = adj.get(n.id);
      if (!neighbors || neighbors.size === 0) continue;

      const counts = new Map<number, number>();
      let best: number | null = null;
      let bestCount = 0;
      for (const nb of neighbors) {
        const l = labels.get(nb);
        if (l === undefined) continue;
        const c = (counts.get(l) ?? 0) + 1;
        counts.set(l, c);
        if (c > bestCount || (c === bestCount && best !== null && l < best)) {
          best = l;
          bestCount = c;
        }
      }

      if (best !== null && best !== labels.get(n.id)) {
        labels.set(n.id, best);
        changed = true;
      }
    }
    return changed;
  };

  for (let i = 0; i < 20; i++) {
    if (!propagate()) break;
  }

  // Remap raw labels to compact 0-based community ids in first-seen order.
  const seen = new Map<number, number>();
  let next = 0;
  const result: CommunityAssignments = new Map();
  for (const n of nodes) {
    const l = labels.get(n.id) ?? 0;
    if (!seen.has(l)) seen.set(l, next++);
    result.set(n.id, seen.get(l) as number);
  }
  return result;
}
