import type { Node } from "@xyflow/react";

export interface ApiNode {
  id?: string;
  label: string;
  kind: string;
}

export interface ApiEdge {
  id?: string;
  source_id?: string;
  target_id?: string;
  source_label: string;
  target_label: string;
  relation: string;
}

export interface GraphDelta {
  nodes: ApiNode[];
  edges: ApiEdge[];
}

export interface IngestRequest {
  text: string;
  nodes: ApiNode[];
  edges: ApiEdge[];
}

/** Data carried by every ReactFlow node on the weave canvas. */
export interface WeaveNodeData extends Record<string, unknown> {
  label: string;
  kind: string;
  fresh: boolean;
  ghost?: boolean;
}

export type WeaveFlowNode = Node<WeaveNodeData, "weave">;

/** Pure knowledge: what exists. No visual or position data. */
export interface KnowledgeNode {
  id: string;
  label: string;
  kind: string;
}

/** Pure knowledge: a relationship between two knowledge nodes by stable id. */
export interface KnowledgeEdge {
  id: string;
  source: string;
  target: string;
  relation: string;
}

export interface XYPosition {
  x: number;
  y: number;
}

/** Transient "Weaving..." indicator shown while an ingest request is in flight. */
export interface GhostNode {
  id: string;
  position: XYPosition;
}
