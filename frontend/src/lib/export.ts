/**
 * Graph export.
 *
 * Weave's knowledge lives in the browser (IndexedDB), so an export is the only
 * thing standing between the user and losing it. Everything here is pure: it
 * takes a snapshot and returns strings, which is what makes it testable
 * without a DOM.
 *
 * The JSON form carries a `schema_version` of its own — independent of the
 * `PersistedScene` version, because the export is a durable artifact that may
 * outlive several in-browser migrations. Bump it only for breaking changes and
 * keep readers tolerant of extra fields.
 */

import type {
  KnowledgeEdge,
  KnowledgeNode,
  SourceRef,
  XYPosition,
} from "./graph-types";

/** Format marker so a file can be identified without guessing. */
export const EXPORT_SCHEMA = "weave.graph";

/** Version of the exported document shape (not of the persisted scene). */
export const EXPORT_SCHEMA_VERSION = 1;

export type ExportFormat = "json" | "obsidian" | "graphml" | "cypher";

export interface ExportInput {
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  positions: Record<string, XYPosition>;
  nodeSources: Record<string, SourceRef>;
  edgeSources: Record<string, SourceRef>;
  communityLabels: Record<string, string>;
  sessionName?: string | null;
}

export interface ExportedFile {
  /** Path inside the archive. */
  path: string;
  content: string;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

function exportedSource(ref: SourceRef | undefined) {
  if (!ref) return null;
  return {
    note: ref.note,
    created_at: new Date(ref.createdAt).toISOString(),
  };
}

function xmlEscape(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

/** Quote a Cypher string literal. Doubling `'` is the safe, injection-proof form. */
function cypherString(value: string): string {
  return `'${value.replace(/\r?\n/g, " ").replace(/'/g, "''")}'`;
}

function confidenceOf(edge: KnowledgeEdge): string {
  return edge.confidence ?? "UNRECORDED";
}

// ---------------------------------------------------------------------------
// JSON — the archival form
// ---------------------------------------------------------------------------

/**
 * Complete, self-contained snapshot: provenance is inlined per node and edge
 * rather than kept in parallel maps, so a reader never has to join tables.
 */
export function exportJson(
  input: ExportInput,
  exportedAt: Date = new Date()
): string {
  const payload = {
    schema: EXPORT_SCHEMA,
    schema_version: EXPORT_SCHEMA_VERSION,
    exported_at: exportedAt.toISOString(),
    session: input.sessionName ?? null,
    counts: { nodes: input.nodes.length, edges: input.edges.length },
    nodes: input.nodes.map((node) => ({
      id: node.id,
      label: node.label,
      kind: node.kind,
      position: input.positions[node.id] ?? null,
      provenance: exportedSource(input.nodeSources[node.id]),
    })),
    edges: input.edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      relation: edge.relation,
      confidence: edge.confidence ?? null,
      evidence: edge.evidence ?? null,
      provenance: exportedSource(input.edgeSources[edge.id]),
    })),
    community_labels: input.communityLabels,
  };

  return `${JSON.stringify(payload, null, 2)}\n`;
}

// ---------------------------------------------------------------------------
// GraphML — for Gephi, yEd, NetworkX, and anything else that reads XML
// ---------------------------------------------------------------------------

export function exportGraphML(input: ExportInput): string {
  const idOf = new Map(input.nodes.map((node) => [node.id, node.id] as const));

  const nodes = input.nodes
    .map((node) => {
      const data = [
        `<data key="d_label">${xmlEscape(node.label)}</data>`,
        node.kind ? `<data key="d_kind">${xmlEscape(node.kind)}</data>` : "",
      ]
        .filter(Boolean)
        .join("");
      return `    <node id="${xmlEscape(node.id)}">${data}</node>`;
    })
    .join("\n");

  const edges = input.edges
    .filter((edge) => idOf.has(edge.source) && idOf.has(edge.target))
    .map((edge, index) => {
      const data = [
        `<data key="d_relation">${xmlEscape(edge.relation)}</data>`,
        `<data key="d_confidence">${xmlEscape(confidenceOf(edge))}</data>`,
        edge.evidence
          ? `<data key="d_evidence">${xmlEscape(edge.evidence)}</data>`
          : "",
      ]
        .filter(Boolean)
        .join("");
      return `    <edge id="e${index + 1}" source="${xmlEscape(edge.source)}" target="${xmlEscape(
        edge.target
      )}">${data}</edge>`;
    })
    .join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">
  <key id="d_label" for="node" attr.name="label" attr.type="string"/>
  <key id="d_kind" for="node" attr.name="kind" attr.type="string"/>
  <key id="d_relation" for="edge" attr.name="relation" attr.type="string"/>
  <key id="d_confidence" for="edge" attr.name="confidence" attr.type="string"/>
  <key id="d_evidence" for="edge" attr.name="evidence" attr.type="string"/>
  <graph id="weave" edgedefault="undirected">
${nodes}
${edges}
  </graph>
</graphml>
`;
}

// ---------------------------------------------------------------------------
// Cypher — for Neo4j / memgraph
// ---------------------------------------------------------------------------

/**
 * Nodes are `:Entity` keyed by Weave id; relationships share the `:RELATED`
 * type and store the human relation as a property, because Cypher types cannot
 * contain spaces.
 */
export function exportCypher(input: ExportInput): string {
  const lines: string[] = [
    `// Weave graph export — schema_version ${EXPORT_SCHEMA_VERSION}`,
    `// ${input.nodes.length} concepts, ${input.edges.length} relationships`,
    "",
  ];

  const known = new Set(input.nodes.map((node) => node.id));

  for (const node of input.nodes) {
    lines.push(
      `MERGE (n:Entity {id: ${cypherString(node.id)}})`,
      `  SET n.label = ${cypherString(node.label)}, n.kind = ${cypherString(node.kind)};`
    );
  }

  if (input.edges.length > 0) lines.push("");

  for (const edge of input.edges) {
    if (!known.has(edge.source) || !known.has(edge.target)) continue;
    lines.push(
      `MATCH (a:Entity {id: ${cypherString(edge.source)}}), (b:Entity {id: ${cypherString(
        edge.target
      )}})`,
      `MERGE (a)-[r:RELATED {relation: ${cypherString(edge.relation)}}]->(b)`,
      `  SET r.confidence = ${cypherString(confidenceOf(edge))};`
    );
  }

  return `${lines.join("\n")}\n`;
}

// ---------------------------------------------------------------------------
// Obsidian — one Markdown note per concept, wikilinked both ways
// ---------------------------------------------------------------------------

const UNSAFE_FILENAME = /[\\/:*?"<>|#^[\]]/g;

function safeFileName(label: string, fallback: string): string {
  const cleaned = label
    .replace(UNSAFE_FILENAME, "-")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^\.+/, "")
    .slice(0, 80)
    .trim();
  return cleaned || fallback;
}

function uniqueFileName(base: string, used: Map<string, number>): string {
  const key = base.toLowerCase();
  const seen = used.get(key) ?? 0;
  used.set(key, seen + 1);
  return seen === 0 ? base : `${base} (${seen + 1})`;
}

/**
 * A vault layout: `Weave.md` index plus `Nodes/<name>.md`. Wikilinks point at
 * the note file names, so the graph is navigable in Obsidian immediately.
 */
export function exportObsidian(
  input: ExportInput,
  exportedAt: Date = new Date()
): ExportedFile[] {
  const used = new Map<string, number>();
  const fileName = new Map<string, string>();
  for (const node of input.nodes) {
    fileName.set(
      node.id,
      uniqueFileName(safeFileName(node.label, node.id), used)
    );
  }

  const byLabel = new Map(input.nodes.map((n) => [n.id, n.label] as const));
  const linkTo = (id: string) =>
    `[[${fileName.get(id) ?? byLabel.get(id) ?? id}]]`;

  const date = exportedAt.toISOString().slice(0, 10);
  const files: ExportedFile[] = [];

  for (const node of input.nodes) {
    const connections = input.edges
      .filter((edge) => edge.source === node.id || edge.target === node.id)
      .map((edge) => {
        const outgoing = edge.source === node.id;
        const otherId = outgoing ? edge.target : edge.source;
        const arrow = outgoing ? "→" : "←";
        const evidence = edge.evidence ? `\n  > ${edge.evidence}` : "";
        return `- ${arrow} ${linkTo(otherId)} — ${edge.relation} — \`${confidenceOf(
          edge
        )}\`${evidence}`;
      });

    const source = input.nodeSources[node.id];
    const body = [
      "---",
      `weave_id: ${node.id}`,
      ...(node.kind ? [`kind: ${node.kind}`] : []),
      "---",
      "",
      `# ${node.label}`,
      "",
      "## Connections",
      "",
      ...(connections.length > 0 ? connections : ["_No relationships yet._"]),
      ...(source?.note
        ? ["", "## Source", "", `> ${source.note.replace(/\n+/g, " ")}`]
        : []),
      "",
      `_Exported from Weave on ${date}._`,
      "",
    ].join("\n");

    files.push({
      path: `Nodes/${fileName.get(node.id)}.md`,
      content: body,
    });
  }

  // Index: a map of the vault, grouped by kind so it reads like a table of
  // contents rather than a flat pile.
  const kinds = new Map<string, KnowledgeNode[]>();
  for (const node of input.nodes) {
    const kind = node.kind || "concept";
    kinds.set(kind, [...(kinds.get(kind) ?? []), node]);
  }

  const index = [
    "# Weave",
    "",
    `Exported ${date} · ${input.nodes.length} concepts · ${input.edges.length} relationships`,
    ...(input.sessionName ? ["", `Session: **${input.sessionName}**`] : []),
    "",
    ...[...kinds.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .flatMap(([kind, nodes]) => [
        `## ${kind}`,
        "",
        ...nodes.map((node) => `- ${linkTo(node.id)}`),
        "",
      ]),
  ].join("\n");

  files.unshift({ path: "Weave.md", content: index });
  return files;
}
