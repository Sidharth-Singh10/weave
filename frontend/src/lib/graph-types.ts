import type { Node } from "@xyflow/react";

export interface ApiNode {
  id?: string;
  label: string;
  kind: string;
}

/** How well an edge is grounded in the note that produced it.
 * Mirrors `weave_core::models::confidence`. */
export type EdgeConfidence = "EXTRACTED" | "INFERRED" | "AMBIGUOUS";

export interface ApiEdge {
  id?: string;
  source_id?: string;
  target_id?: string;
  source_label: string;
  target_label: string;
  relation: string;
  confidence?: EdgeConfidence;
  evidence?: string;
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
  /** Topic-view accent color (kind-based), set when the Topic flavor is active. */
  topicColor?: string;
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
  /** Grounding class. Absent on edges persisted before confidence existed. */
  confidence?: EdgeConfidence;
  /** Sentence from the source note that supports the edge. */
  evidence?: string;
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

/** Metadata for a named, auto-saved project session. */
export interface SessionMeta {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
}

/** Provenance: which note (or manual action) produced a node/edge. */
export interface SourceRef {
  /** The originating note text; null for manually-created items. */
  note: string | null;
  createdAt: number;
}

/** The persistable subset of ViewConfig (transient selection is dropped). */
export interface PersistedViewConfig {
  type: ViewType;
  semanticZoom: SemanticZoomLevel;
}

/** One session's serialized scene: the knowledge graph + view preferences. */
export interface PersistedScene {
  /** Schema version; migrate() bumps this on breaking shape changes. */
  version: 2;
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  positions: Record<string, XYPosition>;
  viewConfig: PersistedViewConfig;
  communityLabels: Record<string, string>;
  /** nodeId -> originating note (provenance). */
  nodeSources: Record<string, SourceRef>;
  /** edgeId -> originating note (provenance). */
  edgeSources: Record<string, SourceRef>;
  savedAt: number;
}
