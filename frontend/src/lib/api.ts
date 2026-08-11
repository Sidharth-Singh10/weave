import type { ApiEdge, GraphDelta, IngestRequest } from "./graph-types";

const API_BASE =
  process.env.NEXT_PUBLIC_WEAVE_API_URL ?? "http://localhost:3001";

export async function ingestNote(req: IngestRequest): Promise<GraphDelta> {
  const res = await fetch(`${API_BASE}/api/graph/ingest`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req),
  });
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(`ingest failed (${res.status})${detail ? `: ${detail}` : ""}`);
  }
  return (await res.json()) as GraphDelta;
}

export interface LabelCommunityResponse {
  label: string;
}

export interface OrganizeGroup {
  label: string;
  member_labels: string[];
}

export interface OrganizeResult {
  groups: OrganizeGroup[];
  missing_edges: ApiEdge[];
  disconnected: string[];
  duplicates: { label_a: string; label_b: string }[];
}

/** Advisory AI analysis of the graph. Always optional — callers degrade
 * gracefully to an empty result on failure. */
export async function organizeGraph(
  nodes: { id?: string; label: string; kind: string }[],
  edges: {
    id?: string;
    source_label: string;
    target_label: string;
    relation: string;
  }[]
): Promise<OrganizeResult> {
  const res = await fetch(`${API_BASE}/api/graph/organize`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nodes, edges }),
  });
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(`organize failed (${res.status})${detail ? `: ${detail}` : ""}`);
  }
  return (await res.json()) as OrganizeResult;
}

/** Ask the LLM to name a community from its member node labels.
 * Returns an empty string on failure — callers fall back to "Group N". */
export async function labelCommunity(nodes: string[]): Promise<string> {
  const res = await fetch(`${API_BASE}/api/graph/label-community`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nodes }),
  });
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(
      `label-community failed (${res.status})${detail ? `: ${detail}` : ""}`
    );
  }
  const data = (await res.json()) as LabelCommunityResponse;
  return data.label;
}
