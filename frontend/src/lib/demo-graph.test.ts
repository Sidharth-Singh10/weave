import { test } from "node:test";
import assert from "node:assert/strict";

import { DEMO_NOTE, DEMO_SESSION_NAME, buildDemoScene, demoPositions } from "./demo-graph.ts";
import type { EdgeConfidence } from "./graph-types.ts";

const CONFIDENCES: EdgeConfidence[] = ["EXTRACTED", "INFERRED", "AMBIGUOUS"];

test("demo scene is a complete, self-consistent graph", () => {
  const scene = buildDemoScene();
  const nodeIds = new Set(scene.nodes.map((node) => node.id));

  assert.equal(scene.version, 2);
  assert.ok(scene.nodes.length >= 20, "enough concepts for a populated canvas");
  assert.ok(scene.edges.length >= 35, "enough edges to show structure");
  assert.equal(new Set(scene.nodes.map((node) => node.id)).size, scene.nodes.length);
  assert.equal(new Set(scene.edges.map((edge) => edge.id)).size, scene.edges.length);

  for (const edge of scene.edges) {
    assert.ok(nodeIds.has(edge.source), `edge ${edge.id} has a known source`);
    assert.ok(nodeIds.has(edge.target), `edge ${edge.id} has a known target`);
    assert.notEqual(edge.source, edge.target);
    assert.ok(edge.relation.trim().length > 0);
    assert.ok(edge.evidence && edge.evidence.trim().length > 0);
    assert.ok(edge.confidence && CONFIDENCES.includes(edge.confidence));
  }
});

test("every node is placed and has provenance", () => {
  const scene = buildDemoScene();

  for (const node of scene.nodes) {
    const position = scene.positions[node.id];
    assert.ok(position, `${node.id} has a position`);
    assert.equal(typeof position.x, "number");
    assert.equal(typeof position.y, "number");
    assert.equal(scene.nodeSources[node.id]?.note, DEMO_NOTE);
    assert.ok(scene.nodeSources[node.id]?.createdAt);
  }

  for (const edge of scene.edges) {
    assert.equal(scene.edgeSources[edge.id]?.note, DEMO_NOTE);
  }
});

test("placement is deterministic and does not overlap", () => {
  const first = demoPositions();
  const second = demoPositions();
  assert.deepEqual(first, second);

  const points = Object.values(first);
  for (let i = 0; i < points.length; i += 1) {
    for (let j = i + 1; j < points.length; j += 1) {
      const distance = Math.hypot(points[i].x - points[j].x, points[i].y - points[j].y);
      assert.ok(distance > 60, `nodes ${i} and ${j} are ${distance.toFixed(0)}px apart`);
    }
  }
});

test("scene survives a JSON round trip", () => {
  const scene = buildDemoScene();
  const restored = JSON.parse(JSON.stringify(scene));

  assert.deepEqual(restored, scene);
  assert.equal(restored.nodes.length, scene.nodes.length);
  assert.deepEqual(restored.communityLabels, {});
  assert.equal(DEMO_SESSION_NAME.length > 0, true);
});

test("each call returns an independent copy", () => {
  const first = buildDemoScene();
  first.nodes[0].label = "mutated";
  first.positions[first.nodes[0].id].x = 9999;

  const second = buildDemoScene();
  assert.notEqual(second.nodes[0].label, "mutated");
  assert.notEqual(second.positions[second.nodes[0].id].x, 9999);
});
