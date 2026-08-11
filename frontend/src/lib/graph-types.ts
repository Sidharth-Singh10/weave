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
  /** Importance score (0-1) from the view engine, drives visual prominence. */
  importance?: number;
  /** Degree centrality from the view engine; high-degree nodes are entry points. */
  degree?: number;
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

export type ViewType = "default" | "topic";

export type SemanticZoomLevel = "overview" | "category" | "entity" | "detail";

/** What the user currently wants to see. Views are projections, not data. */
export interface ViewConfig {
  type: ViewType;
  selectedNodeId?: string;
  /** Neighborhood hop depth when a node is focused (adaptive granularity). */
  focusDepth?: number;
  semanticZoom: SemanticZoomLevel;
}

/** Semantic layout constraints derived from the knowledge graph. */
export interface LayoutContext {
  /** nodeId -> community id. */
  community: Record<string, number>;
  /** community id -> dominant sector angle (radians). */
  communitySector: Record<number, number>;
  /** nodeId -> importance (0-1). */
  importance: Record<string, number>;
}
