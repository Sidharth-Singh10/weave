"use client";

import { create } from "zustand";
import {
  applyEdgeChanges,
  applyNodeChanges,
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
} from "@xyflow/react";
import { ingestNote } from "./api";
import { applyGraphDelta } from "./graph-ops";
import type { GraphDelta, WeaveFlowNode } from "./graph-types";

export type GraphStatus = "idle" | "thinking" | "error";

interface GraphState {
  nodes: WeaveFlowNode[];
  edges: Edge[];
  status: GraphStatus;
  error: string | null;
  onNodesChange: (changes: NodeChange<WeaveFlowNode>[]) => void;
  onEdgesChange: (changes: EdgeChange[]) => void;
  onConnect: (conn: Connection) => void;
  submit: (text: string) => Promise<void>;
  renameNode: (id: string, label: string) => void;
  clearError: () => void;
}

let idCounter = 0;

export const useGraphStore = create<GraphState>()((set, get) => ({
  nodes: [],
  edges: [],
  status: "idle",
  error: null,

  onNodesChange: (changes) =>
    set((s) => ({ nodes: applyNodeChanges(changes, s.nodes) })),

  onEdgesChange: (changes) =>
    set((s) => ({ edges: applyEdgeChanges(changes, s.edges) })),

  onConnect: (conn) =>
    set((s) => {
      if (!conn.source || !conn.target) return s;
      const exists = s.edges.some(
        (e) => e.source === conn.source && e.target === conn.target
      );
      if (exists) return s;
      const edge: Edge = {
        id: `${conn.source}->${conn.target}:${idCounter++}`,
        source: conn.source,
        target: conn.target,
        label: "related to",
        type: "default",
      };
      return { edges: [...s.edges, edge] };
    }),

  renameNode: (id, label) =>
    set((s) => ({
      nodes: s.nodes.map((n) =>
        n.id === id ? { ...n, data: { ...n.data, label } } : n
      ),
    })),

  clearError: () => set({ error: null, status: "idle" }),

  submit: async (text) => {
    const trimmed = text.trim();
    if (!trimmed || get().status === "thinking") return;

    set({ status: "thinking", error: null });

    // Place a ghost node near the centroid so the canvas shows activity.
    const ghostId = `ghost-${Date.now()}`;
    const cur = get().nodes;
    let gx = 0;
    let gy = 0;
    if (cur.length > 0) {
      gx = cur.reduce((s, n) => s + n.position.x, 0) / cur.length + 160;
      gy = cur.reduce((s, n) => s + n.position.y, 0) / cur.length + 40;
    }
    set((s) => ({
      nodes: [
        ...s.nodes,
        {
          id: ghostId,
          type: "weave",
          position: { x: gx, y: gy },
          data: { label: "Weaving...", kind: "", fresh: false, ghost: true },
          selectable: false,
          draggable: false,
        },
      ],
    }));

    const s = get();

    try {
      const delta: GraphDelta = await ingestNote({
        text: trimmed,
        nodes: s.nodes.map((n) => ({
          id: n.id,
          label: n.data.label,
          kind: n.data.kind,
        })),
        edges: s.edges.map((e) => ({
          source_id: e.source,
          target_id: e.target,
          source_label:
            s.nodes.find((n) => n.id === e.source)?.data.label ?? e.source,
          target_label:
            s.nodes.find((n) => n.id === e.target)?.data.label ?? e.target,
          relation: typeof e.label === "string" ? e.label : "related to",
        })),
      });

      set((state) => {
        const liveNodes = state.nodes.filter((n) => !n.data.ghost);
        const { nodes: nextNodes, edges: nextEdges } = applyGraphDelta(
          liveNodes,
          state.edges,
          delta
        );

        return {
          nodes: nextNodes,
          edges: nextEdges,
          status: "idle" as const,
        };
      });

      // Clear the "fresh" pulse after the animation window.
      setTimeout(() => {
        set((state) => ({
          nodes: state.nodes.map((n) =>
            n.data.fresh ? { ...n, data: { ...n.data, fresh: false } } : n
          ),
        }));
      }, 1600);
    } catch (err) {
      set((state) => ({
        nodes: state.nodes.filter((n) => !n.data.ghost),
        status: "error" as const,
        error: err instanceof Error ? err.message : "Something went wrong",
      }));
    }
  },
}));
