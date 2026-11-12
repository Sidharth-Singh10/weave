import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  CANVAS_LEVELS,
  CAPABILITY_CATEGORIES,
  CAPABILITY_GROUPS,
  EXPORT_FORMATS,
  MCP_TOOL_GROUPS,
  PLATFORM_STAGES,
  SPEC_BAR,
} from "./landing-content.ts";
import { EXPORT_DEMO_INPUT } from "./landing-demo-graph.ts";

const here = path.dirname(fileURLToPath(import.meta.url));

test("the MCP tool count matches the marketing claim and the Rust server", () => {
  const tools = MCP_TOOL_GROUPS.flatMap((group) => group.tools);
  assert.equal(tools.length, 18, "the site claims 18 MCP tools");
  assert.equal(new Set(tools.map((tool) => tool.name)).size, 18);

  const serverPath = path.resolve(here, "../../../mcp/src/server.rs");
  if (!existsSync(serverPath)) return;

  const source = readFileSync(serverPath, "utf8");
  const declared = (source.match(/#\[tool\(/g) ?? []).length;
  assert.equal(
    declared,
    tools.length,
    "landing-content must list every tool declared in mcp/src/server.rs"
  );
});

test("export formats are the four real formats, with unique ids", () => {
  assert.deepEqual(
    EXPORT_FORMATS.map((format) => format.id).sort(),
    ["cypher", "graphml", "json", "obsidian"]
  );
});

test("the platform trace lists the nine real pipeline stages", () => {
  assert.deepEqual(
    PLATFORM_STAGES.map((stage) => stage.id),
    [
      "auth",
      "permission",
      "policy",
      "rate_limit",
      "quota",
      "concurrency",
      "extract",
      "usage",
      "response",
    ]
  );
});

test("semantic zoom has exactly four levels, ordered from overview to detail", () => {
  assert.deepEqual(
    CANVAS_LEVELS.map((level) => level.level),
    [0, 1, 2, 3]
  );
  assert.deepEqual(
    CANVAS_LEVELS.map((level) => level.value),
    ["overview", "category", "entity", "detail"]
  );
});

test("the capability index is complete and has no duplicate titles", () => {
  assert.deepEqual(
    CAPABILITY_GROUPS.map((group) => group.category),
    CAPABILITY_CATEGORIES
  );
  for (const group of CAPABILITY_GROUPS) {
    assert.ok(group.items.length > 0, `${group.category} has entries`);
  }

  const titles = CAPABILITY_GROUPS.flatMap((group) =>
    group.items.map((item) => item.title)
  );
  assert.equal(new Set(titles).size, titles.length, "capability titles are unique");
  assert.ok(titles.length >= 40, "the index covers the whole feature set");
});

test("the spec bar has five claims", () => {
  assert.equal(SPEC_BAR.length, 5);
  for (const item of SPEC_BAR) {
    assert.ok(item.value.length > 0);
    assert.ok(item.label.length > 0);
  }
});

test("the export demo has provenance for every node and edge", () => {
  for (const node of EXPORT_DEMO_INPUT.nodes) {
    assert.ok(EXPORT_DEMO_INPUT.nodeSources[node.id], `node ${node.id} source`);
    assert.ok(EXPORT_DEMO_INPUT.positions[node.id], `node ${node.id} position`);
  }
  for (const edge of EXPORT_DEMO_INPUT.edges) {
    assert.ok(EXPORT_DEMO_INPUT.edgeSources[edge.id], `edge ${edge.id} source`);
  }
});
