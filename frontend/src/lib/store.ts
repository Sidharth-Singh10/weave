"use client";

import { create } from "zustand";
import type {
  Connection,
  EdgeChange,
  NodeChange,
} from "@xyflow/react";
import { ingestNote, organizeGraph, ApiError } from "./api";
import type { OrganizeResult } from "./api";
import { applyGraphDelta } from "./graph-ops";
import { computeLayoutContext, relayout } from "./layout";
import type {
  GhostNode,
  GraphDelta,
  KnowledgeEdge,
  KnowledgeNode,
  PersistedScene,
  SessionMeta,
  ViewConfig,
  WeaveFlowNode,
  XYPosition,
} from "./graph-types";

export type GraphStatus = "idle" | "thinking" | "error";

interface GraphState {
  /** Source of truth: the knowledge graph. */
  knowledgeNodes: KnowledgeNode[];
  knowledgeEdges: KnowledgeEdge[];
  /** Visual metadata: persisted node positions, keyed by node id. */
  positions: Record<string, XYPosition>;
  /** Node ids that should render the "fresh" pulse. */
  freshIds: string[];
  /** Transient "Weaving..." indicator while a request is in flight. */
  ghostNode: GhostNode | null;
  /** Which projection of the knowledge graph the user currently sees. */
  viewConfig: ViewConfig;
  /** Community labels keyed by sorted member-id signature. */
  communityLabels: Record<string, string>;
  /** Advisory AI analysis of the graph (optional layer). */
  insights: OrganizeResult | null;
  insightsLoading: boolean;

  status: GraphStatus;
  error: string | null;

  /** Named project sessions (managed by the persistence layer). */
  sessions: SessionMeta[];
  activeSessionId: string | null;
  setSessions: (sessions: SessionMeta[]) => void;
  setActiveSessionId: (id: string | null) => void;
  /** Replace the graph with a saved scene (used on load/session switch). */
  hydrateSession: (scene: PersistedScene) => void;

  onNodesChange: (changes: NodeChange<WeaveFlowNode>[]) => void;
  onEdgesChange: (changes: EdgeChange[]) => void;
  onConnect: (conn: Connection) => void;
  submit: (text: string) => Promise<void>;
  renameNode: (id: string, label: string) => void;
  selectNode: (id: string | undefined) => void;
  relayout: () => void;
  setViewConfig: (config: Partial<ViewConfig>) => void;
  setCommunityLabel: (signature: string, label: string) => void;
  requestInsights: () => Promise<void>;
  clearInsights: () => void;
  clearError: () => void;
}

let idCounter = 0;

export const useGraphStore = create<GraphState>()((set, get) => ({
  knowledgeNodes: [],
  knowledgeEdges: [],
  positions: {},
  freshIds: [],
  ghostNode: null,
  viewConfig: { type: "default", semanticZoom: "entity" },
  communityLabels: {},
  insights: null,
  insightsLoading: false,
  status: "idle",
  error: null,

  sessions: [],
  activeSessionId: null,
  setSessions: (sessions) => set({ sessions }),
  setActiveSessionId: (id) => set({ activeSessionId: id }),

  hydrateSession: (scene) =>
    set(() => ({
      knowledgeNodes: scene.nodes,
      knowledgeEdges: scene.edges,
      positions: scene.positions,
      viewConfig: {
        type: scene.viewConfig.type,
        semanticZoom: scene.viewConfig.semanticZoom,
      },
      communityLabels: scene.communityLabels,
      freshIds: [],
      ghostNode: null,
      insights: null,
      insightsLoading: false,
      status: "idle",
      error: null,
    })),

  onNodesChange: (changes) =>
    set((s) => {
      const removedIds: string[] = [];
      const positionUpdates: { id: string; position: XYPosition }[] = [];

      for (const c of changes) {
        if (c.type === "remove") {
          removedIds.push(c.id);
        } else if (c.type === "position" && c.position) {
          positionUpdates.push({ id: c.id, position: c.position });
        }
      }

      let knowledgeNodes = s.knowledgeNodes;
      let knowledgeEdges = s.knowledgeEdges;
      if (removedIds.length > 0) {
        const removed = new Set(removedIds);
        knowledgeNodes = s.knowledgeNodes.filter((n) => !removed.has(n.id));
        knowledgeEdges = s.knowledgeEdges.filter(
          (e) => !removed.has(e.source) && !removed.has(e.target)
        );
      }

      let positions = s.positions;
      if (positionUpdates.length > 0) {
        positions = { ...s.positions };
        for (const u of positionUpdates) {
          positions[u.id] = u.position;
        }
      }

      return { knowledgeNodes, knowledgeEdges, positions };
    }),

  onEdgesChange: (changes) =>
    set((s) => {
      const removedIds = changes
        .filter((c) => c.type === "remove")
        .map((c) => c.id);
      if (removedIds.length === 0) return s;
      const removed = new Set(removedIds);
      return {
        knowledgeEdges: s.knowledgeEdges.filter((e) => !removed.has(e.id)),
      };
    }),

  onConnect: (conn) =>
    set((s) => {
      if (!conn.source || !conn.target) return s;
      const exists = s.knowledgeEdges.some(
        (e) => e.source === conn.source && e.target === conn.target
      );
      if (exists) return s;
      const edge: KnowledgeEdge = {
        id: `${conn.source}->${conn.target}:${idCounter++}`,
        source: conn.source,
        target: conn.target,
        relation: "related to",
      };
      return { knowledgeEdges: [...s.knowledgeEdges, edge] };
    }),

  renameNode: (id, label) =>
    set((s) => ({
      knowledgeNodes: s.knowledgeNodes.map((n) =>
        n.id === id ? { ...n, label } : n
      ),
    })),

  clearError: () => set({ error: null, status: "idle" }),

  selectNode: (id) =>
    set((s) => ({
      viewConfig: {
        ...s.viewConfig,
        selectedNodeId: id,
        focusDepth: id ? 1 : undefined,
      },
    })),

  setViewConfig: (config) =>
    set((s) => ({ viewConfig: { ...s.viewConfig, ...config } })),

  relayout: () =>
    set((s) => {
      if (s.knowledgeNodes.length === 0) return s;
      return {
        positions: relayout(s.knowledgeNodes, s.knowledgeEdges, s.positions),
      };
    }),

  setCommunityLabel: (signature, label) =>
    set((s) => ({ communityLabels: { ...s.communityLabels, [signature]: label } })),

  requestInsights: async () => {
    const s = get();
    if (s.insightsLoading || s.knowledgeNodes.length === 0) return;
    set({ insightsLoading: true });
    try {
      const insights = await organizeGraph(
        s.knowledgeNodes.map((n) => ({ id: n.id, label: n.label, kind: n.kind })),
        s.knowledgeEdges.map((e) => ({
          source_label:
            s.knowledgeNodes.find((n) => n.id === e.source)?.label ?? e.source,
          target_label:
            s.knowledgeNodes.find((n) => n.id === e.target)?.label ?? e.target,
          relation: e.relation,
        }))
      );
      set({ insights, insightsLoading: false });
    } catch {
      set({ insights: null, insightsLoading: false });
    }
  },

  clearInsights: () => set({ insights: null, insightsLoading: false }),

  submit: async (text) => {
    const trimmed = text.trim();
    if (!trimmed || get().status === "thinking") return;

    set({ status: "thinking", error: null });

    // Place a ghost node near the centroid so the canvas shows activity.
    const ghostId = `ghost-${Date.now()}`;
    const cur = get().knowledgeNodes;
    const pos = get().positions;
    let gx = 0;
    let gy = 0;
    if (cur.length > 0) {
      gx =
        cur.reduce((sum, n) => sum + (pos[n.id]?.x ?? 0), 0) / cur.length + 160;
      gy =
        cur.reduce((sum, n) => sum + (pos[n.id]?.y ?? 0), 0) / cur.length + 40;
    }
    set({
      ghostNode: { id: ghostId, position: { x: gx, y: gy } },
    });

    const s = get();

    try {
      const delta: GraphDelta = await ingestNote({
        text: trimmed,
        nodes: s.knowledgeNodes.map((n) => ({
          id: n.id,
          label: n.label,
          kind: n.kind,
        })),
        edges: s.knowledgeEdges.map((e) => ({
          source_id: e.source,
          target_id: e.target,
          source_label:
            s.knowledgeNodes.find((n) => n.id === e.source)?.label ?? e.source,
          target_label:
            s.knowledgeNodes.find((n) => n.id === e.target)?.label ?? e.target,
          relation: e.relation,
        })),
      });

      set((state) => {
        const layout = computeLayoutContext(
          state.knowledgeNodes,
          state.knowledgeEdges
        );
        const result = applyGraphDelta(
          state.knowledgeNodes,
          state.knowledgeEdges,
          delta,
          state.positions,
          layout
        );

        return {
          knowledgeNodes: result.nodes,
          knowledgeEdges: result.edges,
          positions: { ...state.positions, ...result.positions },
          freshIds: result.newIds,
          ghostNode: null,
          status: "idle" as const,
        };
      });

      // Clear the "fresh" pulse after the animation window.
      setTimeout(() => {
        set({ freshIds: [] });
      }, 1600);
    } catch (err) {
      set({
        ghostNode: null,
        status: "error" as const,
        error: friendlyError(err),
      });
    }
  },
}));

/** Human-readable message for rate-limit / quota errors (§66). */
export function friendlyError(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.code === "quota_exceeded") {
      return "You've reached today's AI usage limit.";
    }
    if (err.code === "rate_limit_exceeded" && err.retryAfter) {
      const secs = Math.max(1, Math.round(err.retryAfter));
      return `You've reached your current usage limit. Try again in ${secs} second${secs === 1 ? "" : "s"}.`;
    }
    if (err.code === "rate_limit_exceeded") {
      return "You've reached your current usage limit.";
    }
  }
  return err instanceof Error ? err.message : "Something went wrong";
}
