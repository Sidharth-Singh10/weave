"use client";

import { useEffect, useMemo, useRef } from "react";
import { labelCommunity } from "./api";
import { detectCommunities } from "./communities";
import { COMMUNITY_COLORS } from "./graph-projection";
import { useGraphStore } from "./store";

export interface CommunityLegendItem {
  id: number;
  signature: string;
  members: string[];
  label: string;
  color: string;
}

/**
 * Derive communities from the knowledge graph and lazily ask the LLM for a
 * name per community (only multi-node communities, only once per member set).
 * Labels are cached by sorted member-id signature so membership changes get a
 * fresh label. Failures degrade to "Group N" — visualization never breaks.
 */
export function useCommunityLabels(): CommunityLegendItem[] {
  const knowledgeNodes = useGraphStore((s) => s.knowledgeNodes);
  const knowledgeEdges = useGraphStore((s) => s.knowledgeEdges);
  const labels = useGraphStore((s) => s.communityLabels);
  const setCommunityLabel = useGraphStore((s) => s.setCommunityLabel);
  const inFlight = useRef<Set<string>>(new Set());

  const items = useMemo(() => {
    const assignments = detectCommunities(knowledgeNodes, knowledgeEdges);
    const groups = new Map<number, string[]>();
    for (const n of knowledgeNodes) {
      const c = assignments.get(n.id) ?? 0;
      const arr = groups.get(c) ?? [];
      arr.push(n.id);
      groups.set(c, arr);
    }

    const byLabel = new Map(knowledgeNodes.map((n) => [n.id, n.label]));
    const result: CommunityLegendItem[] = [];
    let nextId = 0;
    const seen = new Map<number, number>();
    for (const [rawId, memberIds] of groups) {
      if (memberIds.length < 2) continue;
      if (!seen.has(rawId)) seen.set(rawId, nextId++);
      const id = seen.get(rawId) as number;
      result.push({
        id,
        signature: [...memberIds].sort().join("|"),
        members: memberIds.map((id) => byLabel.get(id) ?? id),
        label: "",
        color: COMMUNITY_COLORS[id % COMMUNITY_COLORS.length],
      });
    }
    return result;
  }, [knowledgeNodes, knowledgeEdges]);

  useEffect(() => {
    for (const item of items) {
      if (labels[item.signature] || inFlight.current.has(item.signature)) {
        continue;
      }
      inFlight.current.add(item.signature);
      void labelCommunity(item.members)
        .then((label) =>
          setCommunityLabel(
            item.signature,
            label.trim() || `Group ${item.id + 1}`
          )
        )
        .catch(() => setCommunityLabel(item.signature, `Group ${item.id + 1}`))
        .finally(() => inFlight.current.delete(item.signature));
    }
  }, [items, labels, setCommunityLabel]);

  return items.map((item) => ({
    ...item,
    label: labels[item.signature] ?? `Group ${item.id + 1}`,
  }));
}
