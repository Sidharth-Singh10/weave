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
