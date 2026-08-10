export interface ApiNode {
  label: string;
  kind: string;
}

export interface ApiEdge {
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
