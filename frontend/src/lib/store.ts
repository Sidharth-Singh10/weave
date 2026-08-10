"use client";

import { create } from "zustand";
import {
  applyEdgeChanges,
  applyNodeChanges,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import { ingestNote } from "./api";
import type { GraphDelta } from "./graph-types";

export interface WeaveNodeData extends Record<string, unknown> {
  label: string;
  kind: string;
  fresh: boolean;
  ghost?: boolean;
}

export type WeaveFlowNode = Node<WeaveNodeData, "weave">;

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
function nextId() {
  idCounter += 1;
  return `n${Date.now().toString(36)}-${idCounter}`;
}

function slug(label: string) {
  return label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
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
        nodes: s.nodes.map((n) => ({ label: n.data.label, kind: n.data.kind })),
        edges: s.edges.map((e) => ({
          source_label:
            s.nodes.find((n) => n.id === e.source)?.data.label ?? e.source,
          target_label:
            s.nodes.find((n) => n.id === e.target)?.data.label ?? e.target,
          relation: typeof e.label === "string" ? e.label : "related to",
        })),
      });

      set((state) => {
        // Strip ghost nodes before merging real ones.
        const liveNodes = state.nodes.filter((n) => !n.data.ghost);
        const byLabel = new Map(
          state.nodes.map((n) => [n.data.label.toLowerCase(), n] as const)
        );

        const newNodes: WeaveFlowNode[] = [];
        const newEdges: Edge[] = [];

        // Resolve a label to a node id, tracking nodes added in this pass.
        const resolveId = (
          label: string,
          kind: string
        ): { id: string; isNew: boolean } => {
          const key = label.toLowerCase();
          const existing = byLabel.get(key);
          if (existing) return { id: existing.id, isNew: false };
          const node: WeaveFlowNode = {
            id: nextId(),
            type: "weave",
            position: { x: 0, y: 0 },
            data: { label, kind, fresh: true },
          };
          byLabel.set(key, node);
          newNodes.push(node);
          return { id: node.id, isNew: true };
        };

        // Create nodes first so every edge endpoint exists.
        for (const n of delta.nodes) resolveId(n.label, n.kind);

        // Count how many new nodes attach to each parent for radial layout.
        const parentChildCount = new Map<string, number>();

        for (const e of delta.edges) {
          const src = resolveId(e.source_label, "concept");
          const tgt = resolveId(e.target_label, "concept");
          const edgeId = `${src.id}->${tgt.id}:${slug(e.relation)}`;
          const dup = [...state.edges, ...newEdges].some(
            (x) => x.id === edgeId
          );
          if (!dup && src.id !== tgt.id) {
            newEdges.push({
              id: edgeId,
              source: src.id,
              target: tgt.id,
              label: e.relation,
              type: "default",
            });
            parentChildCount.set(
              src.id,
              (parentChildCount.get(src.id) ?? 0) + 1
            );
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
            ? [...state.nodes, ...newNodes].find((n) => n.id === parentId)
            : undefined;
          const siblings = parentId
            ? (parentChildCount.get(parentId) ?? 1) - 1
            : 0;
          const idx = childIndex.get(parentId ?? "") ?? 0;
          childIndex.set(parentId ?? "", idx + 1);
          node.position = placeNode(parent, siblings, idx);
        }

        return {
          nodes: [...liveNodes, ...newNodes],
          edges: [...state.edges, ...newEdges],
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
