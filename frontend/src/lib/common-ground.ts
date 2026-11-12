"use client";

import { useGraphStore } from "./store";
import type {
  KnowledgeEdge,
  KnowledgeNode,
  SemanticZoomLevel,
  ViewConfig,
  ViewType,
  XYPosition,
} from "./graph-types";

/**
 * Common Ground adapter for Weave. The browser is the adapter: the Rust journal owns ids,
 * revisions, capabilities, and audit; this module applies graph envelopes to the client-owned
 * knowledge graph, returns inverses, and posts acks.
 */

const DOC_KEY = "weave.commonGroundDocId";

/**
 * Common Ground is an optional integration: without its journal service the
 * canvas would render a badge that opens a panel backed by nothing. A deployment
 * that does not run the journal sets `NEXT_PUBLIC_COMMON_GROUND=off`.
 */
export const commonGroundEnabled =
  process.env.NEXT_PUBLIC_COMMON_GROUND !== "off";

export const COMMON_GROUND_BASE = (
  process.env.NEXT_PUBLIC_COMMON_GROUND_URL ?? "/common-ground"
).replace(/\/+$/, "");
const BASE = COMMON_GROUND_BASE;

type GraphOp =
  | { op: "node.create"; id: string; label: string; kind: string; position?: XYPosition }
  | { op: "node.update"; id: string; label?: string; kind?: string; position?: XYPosition }
  | { op: "node.delete"; id: string }
  | { op: "edge.create"; id: string; source: string; target: string; relation: string }
  | { op: "edge.update"; id: string; source?: string; target?: string; relation?: string }
  | { op: "edge.delete"; id: string }
  | { op: "merge"; from: string; into: string }
  | { op: "view.set"; selectedNodeId?: string; semanticZoom?: SemanticZoomLevel; type?: ViewType };

interface Result {
  ok: boolean;
  id?: string;
  warnings?: string[];
}

interface Snapshot {
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  positions: Record<string, XYPosition>;
  viewConfig: ViewConfig;
}

interface Envelope {
  payload?: { op?: unknown };
}

interface ProtocolInverse {
  type: string;
  target: { kind: string; id?: string };
  payload: { op: GraphOp };
}

let started = false;

export function getCommonGroundDocId(): string {
  if (typeof window === "undefined") return "";
  try {
    const existing = window.localStorage.getItem(DOC_KEY);
    if (existing) return existing;
    const id = `weave_${crypto.randomUUID()}`;
    window.localStorage.setItem(DOC_KEY, id);
    return id;
  } catch {
    return `weave_${crypto.randomUUID()}`;
  }
}

function snapshot(): Snapshot {
  const state = useGraphStore.getState();
  return {
    nodes: structuredClone(state.knowledgeNodes),
    edges: structuredClone(state.knowledgeEdges),
    positions: structuredClone(state.positions),
    viewConfig: structuredClone(state.viewConfig),
  };
}

function applyOp(raw: unknown): Result {
  if (typeof raw !== "object" || raw === null) {
    return { ok: false, warnings: ["graph op must be an object"] };
  }
  const op = raw as GraphOp;
  const state = useGraphStore.getState();
  switch (op.op) {
    case "node.create": {
      if (state.knowledgeNodes.some((node) => node.id === op.id)) {
        return { ok: false, warnings: [`node ${op.id} already exists`] };
      }
      useGraphStore.setState((s) => {
        const index = s.knowledgeNodes.length;
        const angle = (Math.PI * 2 * index) / Math.max(index + 1, 3);
        return {
          knowledgeNodes: [...s.knowledgeNodes, { id: op.id, label: op.label, kind: op.kind }],
          positions: {
            ...s.positions,
            [op.id]: op.position ?? { x: 260 * Math.cos(angle), y: 260 * Math.sin(angle) },
          },
          nodeSources: {
            ...s.nodeSources,
            [op.id]: { note: null, createdAt: Date.now() },
          },
        };
      });
      return { ok: true, id: op.id };
    }
    case "node.update": {
      if (!state.knowledgeNodes.some((node) => node.id === op.id)) {
        return { ok: false, warnings: [`no node ${op.id}`] };
      }
      useGraphStore.setState((s) => ({
        knowledgeNodes: s.knowledgeNodes.map((node) =>
          node.id === op.id
            ? {
                ...node,
                label: op.label ?? node.label,
                kind: op.kind ?? node.kind,
              }
            : node,
        ),
        positions: op.position ? { ...s.positions, [op.id]: op.position } : s.positions,
      }));
      return { ok: true, id: op.id };
    }
    case "node.delete": {
      if (!state.knowledgeNodes.some((node) => node.id === op.id)) {
        return { ok: false, warnings: [`no node ${op.id}`] };
      }
      const removed = new Set([op.id]);
      useGraphStore.setState((s) => ({
        knowledgeNodes: s.knowledgeNodes.filter((node) => !removed.has(node.id)),
        knowledgeEdges: s.knowledgeEdges.filter(
          (edge) => !removed.has(edge.source) && !removed.has(edge.target),
        ),
        positions: Object.fromEntries(
          Object.entries(s.positions).filter(([id]) => !removed.has(id)),
        ),
        nodeSources: Object.fromEntries(
          Object.entries(s.nodeSources).filter(([id]) => !removed.has(id)),
        ),
        edgeSources: Object.fromEntries(
          Object.entries(s.edgeSources).filter(([id]) => {
            const edge = s.knowledgeEdges.find((candidate) => candidate.id === id);
            return edge && !removed.has(edge.source) && !removed.has(edge.target);
          }),
        ),
      }));
      return { ok: true, id: op.id };
    }
    case "edge.create": {
      if (op.source === op.target) {
        return { ok: false, warnings: ["an edge needs two different nodes"] };
      }
      if (state.knowledgeEdges.some((edge) => edge.id === op.id)) {
        return { ok: false, warnings: [`edge ${op.id} already exists`] };
      }
      if (
        !state.knowledgeNodes.some((node) => node.id === op.source) ||
        !state.knowledgeNodes.some((node) => node.id === op.target)
      ) {
        return { ok: false, warnings: ["edge endpoints must exist"] };
      }
      useGraphStore.setState((s) => ({
        knowledgeEdges: [
          ...s.knowledgeEdges,
          { id: op.id, source: op.source, target: op.target, relation: op.relation },
        ],
        edgeSources: {
          ...s.edgeSources,
          [op.id]: { note: null, createdAt: Date.now() },
        },
      }));
      return { ok: true, id: op.id };
    }
    case "edge.update": {
      const edge = state.knowledgeEdges.find((candidate) => candidate.id === op.id);
      if (!edge) return { ok: false, warnings: [`no edge ${op.id}`] };
      const source = op.source ?? edge.source;
      const target = op.target ?? edge.target;
      if (source === target) {
        return { ok: false, warnings: ["an edge needs two different nodes"] };
      }
      if (
        !state.knowledgeNodes.some((node) => node.id === source) ||
        !state.knowledgeNodes.some((node) => node.id === target)
      ) {
        return { ok: false, warnings: ["edge endpoints must exist"] };
      }
      useGraphStore.setState((s) => ({
        knowledgeEdges: s.knowledgeEdges.map((candidate) =>
          candidate.id === op.id
            ? {
                ...candidate,
                source,
                target,
                relation: op.relation ?? candidate.relation,
              }
            : candidate,
        ),
      }));
      return { ok: true, id: op.id };
    }
    case "edge.delete": {
      if (!state.knowledgeEdges.some((edge) => edge.id === op.id)) {
        return { ok: false, warnings: [`no edge ${op.id}`] };
      }
      useGraphStore.setState((s) => ({
        knowledgeEdges: s.knowledgeEdges.filter((edge) => edge.id !== op.id),
        edgeSources: Object.fromEntries(
          Object.entries(s.edgeSources).filter(([id]) => id !== op.id),
        ),
      }));
      return { ok: true, id: op.id };
    }
    case "merge": {
      if (op.from === op.into) {
        return { ok: false, warnings: ["cannot merge a node into itself"] };
      }
      if (
        !state.knowledgeNodes.some((node) => node.id === op.from) ||
        !state.knowledgeNodes.some((node) => node.id === op.into)
      ) {
        return { ok: false, warnings: ["merge endpoints must exist"] };
      }
      const seen = new Set<string>();
      useGraphStore.setState((s) => ({
        knowledgeNodes: s.knowledgeNodes.filter((node) => node.id !== op.from),
        knowledgeEdges: s.knowledgeEdges
          .map((edge) => ({
            ...edge,
            source: edge.source === op.from ? op.into : edge.source,
            target: edge.target === op.from ? op.into : edge.target,
          }))
          .filter((edge) => {
            if (edge.source === edge.target) return false;
            const key = `${edge.source}|${edge.target}|${edge.relation}`;
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
          }),
        positions: Object.fromEntries(
          Object.entries(s.positions).filter(([id]) => id !== op.from),
        ),
        nodeSources: Object.fromEntries(
          Object.entries(s.nodeSources).filter(([id]) => id !== op.from),
        ),
      }));
      return { ok: true, id: op.into };
    }
    case "view.set": {
      useGraphStore.setState((s) => ({
        viewConfig: {
          ...s.viewConfig,
          ...(op.type ? { type: op.type } : {}),
          ...(op.selectedNodeId !== undefined ? { selectedNodeId: op.selectedNodeId } : {}),
          ...(op.semanticZoom ? { semanticZoom: op.semanticZoom } : {}),
        },
      }));
      return { ok: true };
    }
    default:
      return { ok: false, warnings: [`unknown graph op ${(op as { op?: string }).op ?? "?"}`] };
  }
}

function inverseOf(before: Snapshot, after: Snapshot): GraphOp[] {
  const inverses: GraphOp[] = [];
  const beforeNodes = new Map(before.nodes.map((node) => [node.id, node]));
  const afterNodes = new Map(after.nodes.map((node) => [node.id, node]));
  const beforeEdges = new Map(before.edges.map((edge) => [edge.id, edge]));
  const afterEdges = new Map(after.edges.map((edge) => [edge.id, edge]));

  for (const node of after.nodes) {
    if (!beforeNodes.has(node.id)) inverses.push({ op: "node.delete", id: node.id });
  }
  for (const edge of after.edges) {
    if (!beforeEdges.has(edge.id)) inverses.push({ op: "edge.delete", id: edge.id });
  }
  for (const edge of before.edges) {
    const next = afterEdges.get(edge.id);
    if (
      next &&
      (next.source !== edge.source || next.target !== edge.target || next.relation !== edge.relation)
    ) {
      inverses.push({
        op: "edge.update",
        id: edge.id,
        source: edge.source,
        target: edge.target,
        relation: edge.relation,
      });
    }
  }
  for (const node of before.nodes) {
    const next = afterNodes.get(node.id);
    const position = before.positions[node.id];
    if (next && (next.label !== node.label || next.kind !== node.kind)) {
      inverses.push({
        op: "node.update",
        id: node.id,
        label: node.label,
        kind: node.kind,
        ...(position ? { position } : {}),
      });
    }
  }
  for (const edge of before.edges) {
    if (!afterEdges.has(edge.id)) {
      inverses.push({
        op: "edge.create",
        id: edge.id,
        source: edge.source,
        target: edge.target,
        relation: edge.relation,
      });
    }
  }
  for (const node of before.nodes) {
    if (!afterNodes.has(node.id)) {
      const position = before.positions[node.id];
      inverses.push({
        op: "node.create",
        id: node.id,
        label: node.label,
        kind: node.kind,
        ...(position ? { position } : {}),
      });
    }
  }
  return inverses;
}

function toInverseOps(ops: GraphOp[]): ProtocolInverse[] {
  return ops.map((op) => {
    const record = op as { id?: string; into?: string };
    const kind = op.op.startsWith("node.")
      ? "graph.node"
      : op.op.startsWith("edge.")
        ? "graph.edge"
        : "graph";
    const id = record.id ?? record.into;
    return {
      type: `graph.${op.op}`,
      target: { kind, ...(id ? { id } : {}) },
      payload: { op },
    };
  });
}

async function ack(docId: string, opId: string, body: Record<string, unknown>): Promise<void> {
  try {
    await fetch(`${BASE}/api/docs/${encodeURIComponent(docId)}/acks`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ acks: [{ op_id: opId, ...body }] }),
    });
  } catch (error) {
    console.warn("[common-ground] ack failed", error);
  }
}

async function handle(docId: string, event: MessageEvent): Promise<void> {
  let parsed: { type?: string; data?: { docId?: string; opId?: string; envelope?: Envelope } };
  try {
    parsed = JSON.parse(event.data);
  } catch {
    return;
  }
  if (parsed.type !== "doc.op.request" || parsed.data?.docId !== docId) return;
  const opId = parsed.data.opId;
  const raw = parsed.data.envelope?.payload?.op;
  if (!opId || raw === undefined) return;

  const before = snapshot();
  const result = applyOp(raw);
  const after = snapshot();
  const inverse = result.ok ? toInverseOps(inverseOf(before, after)) : [];
  const opName =
    typeof raw === "object" && raw !== null && typeof (raw as { op?: unknown }).op === "string"
      ? (raw as { op: string }).op
      : "";
  const known = [
    "node.create",
    "node.update",
    "node.delete",
    "edge.create",
    "edge.update",
    "edge.delete",
    "merge",
    "view.set",
  ].includes(opName);
  await ack(docId, opId, {
    status: result.ok ? "applied" : "rejected",
    result,
    inverse,
    snapshot: after,
    ...(result.ok
      ? {}
      : {
          rejected_by: "adapter",
          code: known ? "invalid_op" : "unknown_op_type",
          message: result.warnings?.join("; ") || "graph adapter rejected the op",
          retryable: false,
        }),
  });
}

/** Starts the SSE bridge once. Safe to call from a React effect. */
export function startCommonGroundBridge(): () => void {
  if (typeof window === "undefined" || started) return () => {};
  started = true;
  const docId = getCommonGroundDocId();
  const source = new EventSource(`${BASE}/api/stream`);
  source.onmessage = (event) => {
    void handle(docId, event);
  };
  return () => {
    started = false;
    source.close();
  };
}
