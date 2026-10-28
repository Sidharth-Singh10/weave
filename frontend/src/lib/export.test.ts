import { test } from "node:test";
import assert from "node:assert/strict";

import {
  EXPORT_SCHEMA,
  EXPORT_SCHEMA_VERSION,
  exportCypher,
  exportGraphML,
  exportJson,
  exportObsidian,
  type ExportInput,
} from "./export.ts";
import type { KnowledgeEdge, KnowledgeNode } from "./graph-types.ts";

const AT = new Date("2026-10-28T09:30:00.000Z");

function node(id: string, label: string, kind = "person"): KnowledgeNode {
  return { id, label, kind };
}

function edge(
  id: string,
  source: string,
  target: string,
  relation: string,
  confidence?: KnowledgeEdge["confidence"],
  evidence?: string
): KnowledgeEdge {
  return { id, source, target, relation, confidence, evidence };
}

function fixture(): ExportInput {
  return {
    nodes: [
      node("node-harry", "Harry Potter"),
      node("node-hogwarts", "Hogwarts", "place"),
      node("node-ron", "Ron Weasley"),
    ],
    edges: [
      edge("e1", "node-harry", "node-hogwarts", "studies at", "EXTRACTED", "Harry studies at Hogwarts."),
      edge("e2", "node-harry", "node-ron", "friend of", "INFERRED"),
    ],
    positions: { "node-harry": { x: 10, y: 20 } },
    nodeSources: {
      "node-harry": { note: "Harry studies at Hogwarts.", createdAt: AT.getTime() },
    },
    edgeSources: {},
    communityLabels: { "node-harry|node-ron": "Gryffindor" },
    sessionName: "Hogwarts",
  };
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

test("json export carries schema, version, and counts", () => {
  const parsed = JSON.parse(exportJson(fixture(), AT));

  assert.equal(parsed.schema, EXPORT_SCHEMA);
  assert.equal(parsed.schema_version, EXPORT_SCHEMA_VERSION);
  assert.equal(parsed.exported_at, AT.toISOString());
  assert.equal(parsed.session, "Hogwarts");
  assert.deepEqual(parsed.counts, { nodes: 3, edges: 2 });
});

test("json export inlines provenance and position per node", () => {
  const parsed = JSON.parse(exportJson(fixture(), AT));
  const harry = parsed.nodes.find((n: { id: string }) => n.id === "node-harry");

  assert.deepEqual(harry.position, { x: 10, y: 20 });
  assert.equal(harry.provenance.note, "Harry studies at Hogwarts.");
  assert.equal(harry.provenance.created_at, AT.toISOString());

  const ron = parsed.nodes.find((n: { id: string }) => n.id === "node-ron");
  assert.equal(ron.position, null);
  assert.equal(ron.provenance, null);
});

test("json export keeps edge confidence, evidence, and legacy nulls", () => {
  const input = fixture();
  input.edges.push(edge("e3", "node-ron", "node-hogwarts", "related to"));

  const parsed = JSON.parse(exportJson(input, AT));
  const extracted = parsed.edges.find((e: { id: string }) => e.id === "e1");
  const legacy = parsed.edges.find((e: { id: string }) => e.id === "e3");

  assert.equal(extracted.confidence, "EXTRACTED");
  assert.equal(extracted.evidence, "Harry studies at Hogwarts.");
  assert.equal(legacy.confidence, null, "edges from before confidence must not claim a class");
  assert.equal(legacy.evidence, null);
});

test("json export is valid JSON with a trailing newline", () => {
  const text = exportJson(fixture(), AT);
  assert.ok(text.endsWith("\n"));
  assert.doesNotThrow(() => JSON.parse(text));
});

test("json export handles an empty graph", () => {
  const empty: ExportInput = {
    nodes: [],
    edges: [],
    positions: {},
    nodeSources: {},
    edgeSources: {},
    communityLabels: {},
  };
  const parsed = JSON.parse(exportJson(empty, AT));
  assert.deepEqual(parsed.counts, { nodes: 0, edges: 0 });
  assert.deepEqual(parsed.nodes, []);
});

// ---------------------------------------------------------------------------
// GraphML
// ---------------------------------------------------------------------------

test("graphml escapes XML in labels and evidence", () => {
  const input = fixture();
  input.nodes[0] = node("node-amp", "R&D <team>", "org");

  const xml = exportGraphML(input);
  assert.ok(xml.includes("R&amp;D &lt;team&gt;"));
  assert.ok(!xml.includes("<team>"));
});

test("graphml declares keys and emits one edge per relation", () => {
  const xml = exportGraphML(fixture());

  for (const key of ["d_label", "d_kind", "d_relation", "d_confidence"]) {
    assert.ok(xml.includes(`<key id="${key}"`), `missing key ${key}`);
  }
  assert.equal((xml.match(/<edge /g) ?? []).length, 2);
  assert.equal((xml.match(/<node /g) ?? []).length, 3);
  assert.ok(xml.includes("<graph id=\"weave\" edgedefault=\"undirected\">"));
});

test("graphml drops edges whose endpoints are missing", () => {
  const input = fixture();
  input.edges.push(edge("e9", "node-harry", "node-ghost", "haunts"));

  const xml = exportGraphML(input);
  assert.equal((xml.match(/<edge /g) ?? []).length, 2);
  assert.ok(!xml.includes("node-ghost"));
});

// ---------------------------------------------------------------------------
// Cypher
// ---------------------------------------------------------------------------

test("cypher escapes quotes and flattens newlines", () => {
  const input = fixture();
  input.nodes[0] = node("node-obrien", "O'Brien\nSr");

  const cypher = exportCypher(input);
  assert.ok(cypher.includes("'O''Brien Sr'"), "single quotes must be doubled");
  assert.ok(!cypher.includes("O'Brien"));
});

test("cypher merges nodes then relationships", () => {
  const cypher = exportCypher(fixture());

  assert.equal((cypher.match(/MERGE \(n:Entity/g) ?? []).length, 3);
  assert.equal((cypher.match(/\[r:RELATED/g) ?? []).length, 2);
  assert.ok(cypher.includes("schema_version 1"));
});

test("cypher skips relationships with unknown endpoints", () => {
  const input = fixture();
  input.edges.push(edge("e9", "node-harry", "node-ghost", "haunts"));

  const cypher = exportCypher(input);
  assert.equal((cypher.match(/\[r:RELATED/g) ?? []).length, 2);
});

// ---------------------------------------------------------------------------
// Obsidian
// ---------------------------------------------------------------------------

test("obsidian emits an index plus one note per concept", () => {
  const files = exportObsidian(fixture(), AT);

  assert.equal(files.length, 4, "3 nodes + index");
  assert.equal(files[0].path, "Weave.md");
  assert.ok(files.some((f) => f.path === "Nodes/Harry Potter.md"));
  assert.ok(files.some((f) => f.path === "Nodes/Hogwarts.md"));
});

test("obsidian wikilinks match the note file names", () => {
  const files = exportObsidian(fixture(), AT);
  const harry = files.find((f) => f.path === "Nodes/Harry Potter.md");
  const ron = files.find((f) => f.path === "Nodes/Ron Weasley.md");

  assert.ok(harry, "harry note missing");
  assert.ok(harry.content.includes("→ [[Hogwarts]] — studies at — `EXTRACTED`"));
  assert.ok(harry.content.includes("→ [[Ron Weasley]] — friend of — `INFERRED`"));
  assert.ok(harry.content.includes("> Harry studies at Hogwarts."));

  // The same edge is rendered as incoming from the other endpoint.
  assert.ok(ron?.content.includes("← [[Harry Potter]] — friend of — `INFERRED`"));
});

test("obsidian note includes frontmatter and kind", () => {
  const files = exportObsidian(fixture(), AT);
  const hogwarts = files.find((f) => f.path === "Nodes/Hogwarts.md");

  assert.ok(hogwarts?.content.startsWith("---\nweave_id: node-hogwarts\nkind: place\n---"));
  assert.ok(hogwarts?.content.includes("_Exported from Weave on 2026-10-28._"));
});

test("obsidian sanitizes path separators in labels", () => {
  const input = fixture();
  input.nodes.push(node("node-path", "A/B: C", "concept"));

  const files = exportObsidian(input, AT);
  const written = files.find((f) => f.content.includes("A/B: C"));
  assert.ok(written, "note for the node must exist");
  const base = written.path.replace(/^Nodes\//, "");
  assert.ok(!base.includes("/"), `file name must not contain a separator: ${base}`);
  assert.ok(!base.includes(":"), `file name must not contain a colon: ${base}`);
});

test("obsidian disambiguates duplicate labels instead of overwriting", () => {
  const input = fixture();
  input.nodes.push(node("node-mercury-1", "Mercury", "concept"));
  input.nodes.push(node("node-mercury-2", "Mercury", "concept"));

  const files = exportObsidian(input, AT);
  const paths = files.map((f) => f.path);

  assert.ok(paths.includes("Nodes/Mercury.md"));
  assert.ok(paths.includes("Nodes/Mercury (2).md"));
  assert.equal(new Set(paths).size, paths.length, "paths must be unique");
});

test("obsidian marks unconnected concepts explicitly", () => {
  const input = fixture();
  input.nodes.push(node("node-isolated", "Isolated", "concept"));

  const files = exportObsidian(input, AT);
  const isolated = files.find((f) => f.path === "Nodes/Isolated.md");
  assert.ok(isolated?.content.includes("_No relationships yet._"));
});
