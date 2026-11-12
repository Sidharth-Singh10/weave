import type {
  EdgeConfidence,
  KnowledgeEdge,
  KnowledgeNode,
  PersistedScene,
  XYPosition,
} from "./graph-types";

/**
 * The first-paint graph for the public demo.
 *
 * A demo visitor has typed nothing, so the canvas would otherwise be empty. This
 * module ships one small, coherent knowledge graph — the Apollo program — with
 * authored positions, provenance, edge confidence and community labels, so the
 * node inspector, insights panel and every export format have something real to
 * show without a single keystroke.
 *
 * Everything here is deterministic: same ids, same coordinates, same blob on
 * every load.
 */

/** Fixed timestamp so the seeded blob never shifts between runs. */
const SEEDED_AT = Date.UTC(2026, 0, 1);

/** The note every seeded node and edge claims as its source. */
export const DEMO_NOTE = `# Apollo program
The Apollo program was run by NASA from 1961 to 1972, in the middle of the Space Race.
Kennedy's 1962 speech set the goal for the program and escalated the Space Race.
Saturn V was designed by Wernher von Braun, who led the Marshall Space Flight Center at NASA.
Apollo 1 tested the command module and was reviewed by NASA after its fire.
Apollo 8 orbited the Moon ahead of the first landing and flew the command module.
Apollo 11 achieved the first Moon landing in the lunar module, flew the command module, and began lunar sample return.
Neil Armstrong commanded Apollo 11; Buzz Aldrin was lunar module pilot and Michael Collins was command module pilot.
The Apollo Guidance Computer was built by the MIT Instrumentation Laboratory and flew on Apollo 11.
Margaret Hamilton led development of the Apollo Guidance Computer at the MIT Instrumentation Laboratory.
The guidance computer assisted the landing.
Apollo 13 was commanded by Jim Lovell and suffered an explosion in the command module.
The crew used the lunar module as a lifeboat, and the mission was directed by Gene Kranz.
Gene Kranz led mission control, which supported the Apollo program.
Flight dynamics was practiced at mission control and guided Apollo 13 home.
Apollo 17 closed out the Moon landing, returned samples during lunar sample return, which the program produced.
The command module docked with the lunar module, which enabled lunar sample return.
The program relied on Margaret Hamilton's team; Saturn V symbolised the Space Race.`;

interface SeedNode {
  id: string;
  label: string;
  kind: string;
  /** Cluster used for layout and community labels. */
  community: number;
}

interface SeedEdge {
  source: string;
  target: string;
  relation: string;
  confidence: EdgeConfidence;
  evidence: string;
}

const NODES: SeedNode[] = [
  // Program & politics
  { id: "apollo-program", label: "Apollo Program", kind: "project", community: 0 },
  { id: "nasa", label: "NASA", kind: "organization", community: 0 },
  { id: "space-race", label: "Space Race", kind: "concept", community: 0 },
  { id: "kennedy-speech", label: "Kennedy's Moon Speech", kind: "event", community: 0 },
  { id: "apollo-1", label: "Apollo 1", kind: "mission", community: 0 },

  // People
  { id: "neil-armstrong", label: "Neil Armstrong", kind: "person", community: 1 },
  { id: "buzz-aldrin", label: "Buzz Aldrin", kind: "person", community: 1 },
  { id: "michael-collins", label: "Michael Collins", kind: "person", community: 1 },
  { id: "jim-lovell", label: "Jim Lovell", kind: "person", community: 1 },
  { id: "gene-kranz", label: "Gene Kranz", kind: "person", community: 1 },
  { id: "margaret-hamilton", label: "Margaret Hamilton", kind: "person", community: 1 },
  { id: "wernher-von-braun", label: "Wernher von Braun", kind: "person", community: 1 },

  // Missions
  { id: "apollo-8", label: "Apollo 8", kind: "mission", community: 2 },
  { id: "apollo-11", label: "Apollo 11", kind: "mission", community: 2 },
  { id: "apollo-13", label: "Apollo 13", kind: "mission", community: 2 },
  { id: "apollo-17", label: "Apollo 17", kind: "mission", community: 2 },

  // Hardware & software
  { id: "saturn-v", label: "Saturn V", kind: "technology", community: 3 },
  { id: "apollo-guidance-computer", label: "Apollo Guidance Computer", kind: "technology", community: 3 },
  { id: "mit-instrumentation-lab", label: "MIT Instrumentation Laboratory", kind: "organization", community: 3 },
  { id: "command-module", label: "Command Module", kind: "spacecraft", community: 3 },
  { id: "lunar-module", label: "Lunar Module", kind: "spacecraft", community: 3 },

  // Flight operations
  { id: "mission-control", label: "Mission Control", kind: "organization", community: 4 },
  { id: "flight-dynamics", label: "Flight Dynamics", kind: "concept", community: 4 },
  { id: "moon-landing", label: "Moon Landing", kind: "event", community: 4 },
  { id: "lunar-sample-return", label: "Lunar Sample Return", kind: "concept", community: 4 },
];

const EDGES: SeedEdge[] = [
  { source: "apollo-program", target: "nasa", relation: "run by", confidence: "EXTRACTED", evidence: "The Apollo program was run by NASA from 1961 to 1972." },
  { source: "apollo-program", target: "saturn-v", relation: "launched on", confidence: "EXTRACTED", evidence: "Saturn V was designed by Wernher von Braun." },
  { source: "apollo-program", target: "apollo-1", relation: "included", confidence: "EXTRACTED", evidence: "Apollo 1 tested the command module." },
  { source: "apollo-program", target: "apollo-8", relation: "included", confidence: "EXTRACTED", evidence: "Apollo 8 orbited the Moon ahead of the first landing." },
  { source: "apollo-program", target: "apollo-11", relation: "included", confidence: "EXTRACTED", evidence: "Apollo 11 achieved the first Moon landing." },
  { source: "apollo-program", target: "apollo-13", relation: "included", confidence: "EXTRACTED", evidence: "Apollo 13 was commanded by Jim Lovell." },
  { source: "apollo-program", target: "apollo-17", relation: "included", confidence: "EXTRACTED", evidence: "Apollo 17 closed out the Moon landing." },
  { source: "apollo-program", target: "space-race", relation: "was part of", confidence: "EXTRACTED", evidence: "The Apollo program ran in the middle of the Space Race." },
  { source: "space-race", target: "nasa", relation: "motivated", confidence: "INFERRED", evidence: "The program ran in the middle of the Space Race." },
  { source: "kennedy-speech", target: "apollo-program", relation: "set the goal for", confidence: "EXTRACTED", evidence: "Kennedy's 1962 speech set the goal for the program." },
  { source: "kennedy-speech", target: "space-race", relation: "escalated", confidence: "EXTRACTED", evidence: "Kennedy's speech escalated the Space Race." },
  { source: "saturn-v", target: "wernher-von-braun", relation: "designed by", confidence: "EXTRACTED", evidence: "Saturn V was designed by Wernher von Braun." },
  { source: "wernher-von-braun", target: "nasa", relation: "led Marshall Space Flight Center at", confidence: "EXTRACTED", evidence: "Von Braun led the Marshall Space Flight Center at NASA." },
  { source: "apollo-11", target: "neil-armstrong", relation: "commanded by", confidence: "EXTRACTED", evidence: "Neil Armstrong commanded Apollo 11." },
  { source: "apollo-11", target: "buzz-aldrin", relation: "lunar module pilot", confidence: "EXTRACTED", evidence: "Buzz Aldrin was lunar module pilot." },
  { source: "apollo-11", target: "michael-collins", relation: "command module pilot", confidence: "EXTRACTED", evidence: "Michael Collins was command module pilot." },
  { source: "apollo-11", target: "moon-landing", relation: "achieved", confidence: "EXTRACTED", evidence: "Apollo 11 achieved the first Moon landing." },
  { source: "apollo-11", target: "command-module", relation: "flew", confidence: "EXTRACTED", evidence: "Apollo 11 flew the command module." },
  { source: "apollo-11", target: "lunar-module", relation: "landed in", confidence: "EXTRACTED", evidence: "Apollo 11 landed in the lunar module." },
  { source: "command-module", target: "lunar-module", relation: "docked with", confidence: "EXTRACTED", evidence: "The command module docked with the lunar module." },
  { source: "apollo-guidance-computer", target: "mit-instrumentation-lab", relation: "built by", confidence: "EXTRACTED", evidence: "The Apollo Guidance Computer was built by the MIT Instrumentation Laboratory." },
  { source: "margaret-hamilton", target: "mit-instrumentation-lab", relation: "directed software at", confidence: "EXTRACTED", evidence: "Margaret Hamilton led development at the MIT Instrumentation Laboratory." },
  { source: "margaret-hamilton", target: "apollo-guidance-computer", relation: "led development of", confidence: "EXTRACTED", evidence: "Margaret Hamilton led development of the Apollo Guidance Computer." },
  { source: "apollo-guidance-computer", target: "apollo-11", relation: "flew on", confidence: "EXTRACTED", evidence: "The Apollo Guidance Computer flew on Apollo 11." },
  { source: "apollo-guidance-computer", target: "moon-landing", relation: "assisted", confidence: "INFERRED", evidence: "The guidance computer assisted the landing." },
  { source: "apollo-8", target: "command-module", relation: "flew", confidence: "EXTRACTED", evidence: "Apollo 8 flew the command module." },
  { source: "apollo-8", target: "jim-lovell", relation: "crewed by", confidence: "EXTRACTED", evidence: "Apollo 8 orbited the Moon ahead of the first landing." },
  { source: "apollo-8", target: "moon-landing", relation: "orbited the Moon ahead of", confidence: "EXTRACTED", evidence: "Apollo 8 orbited the Moon ahead of the first landing." },
  { source: "apollo-13", target: "jim-lovell", relation: "commanded by", confidence: "EXTRACTED", evidence: "Apollo 13 was commanded by Jim Lovell." },
  { source: "apollo-13", target: "command-module", relation: "suffered an explosion in", confidence: "EXTRACTED", evidence: "Apollo 13 suffered an explosion in the command module." },
  { source: "apollo-13", target: "lunar-module", relation: "used as a lifeboat", confidence: "EXTRACTED", evidence: "The crew used the lunar module as a lifeboat." },
  { source: "apollo-13", target: "gene-kranz", relation: "directed by", confidence: "EXTRACTED", evidence: "Apollo 13 was directed by Gene Kranz." },
  { source: "gene-kranz", target: "mission-control", relation: "led", confidence: "EXTRACTED", evidence: "Gene Kranz led mission control." },
  { source: "mission-control", target: "apollo-program", relation: "supported", confidence: "EXTRACTED", evidence: "Mission control supported the Apollo program." },
  { source: "flight-dynamics", target: "mission-control", relation: "practiced at", confidence: "EXTRACTED", evidence: "Flight dynamics was practiced at mission control." },
  { source: "flight-dynamics", target: "apollo-13", relation: "guided home", confidence: "INFERRED", evidence: "Flight dynamics guided Apollo 13 home." },
  { source: "apollo-17", target: "moon-landing", relation: "closed out", confidence: "EXTRACTED", evidence: "Apollo 17 closed out the Moon landing." },
  { source: "apollo-17", target: "lunar-sample-return", relation: "returned samples during", confidence: "EXTRACTED", evidence: "Apollo 17 returned samples during lunar sample return." },
  { source: "lunar-sample-return", target: "apollo-program", relation: "produced", confidence: "EXTRACTED", evidence: "Lunar sample return is what the program produced." },
  { source: "apollo-1", target: "command-module", relation: "tested", confidence: "EXTRACTED", evidence: "Apollo 1 tested the command module." },
  { source: "apollo-1", target: "nasa", relation: "reviewed by", confidence: "EXTRACTED", evidence: "Apollo 1 was reviewed by NASA after its fire." },
  { source: "apollo-program", target: "margaret-hamilton", relation: "relied on", confidence: "INFERRED", evidence: "The program relied on Margaret Hamilton's team." },
  { source: "saturn-v", target: "space-race", relation: "symbolised", confidence: "AMBIGUOUS", evidence: "Saturn V symbolised the Space Race." },
  { source: "lunar-module", target: "lunar-sample-return", relation: "enabled", confidence: "INFERRED", evidence: "The lunar module enabled lunar sample return." },
  { source: "apollo-11", target: "lunar-sample-return", relation: "began", confidence: "EXTRACTED", evidence: "Apollo 11 began lunar sample return." },
];

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

const COMMUNITY_COUNT = 5;
const CLUSTER_RING = 300;
const MEMBER_RING = 110;
const MIN_NODE_DISTANCE = 125;
const IDEAL_EDGE_LENGTH = 165;
const FORCE_ITERATIONS = 320;
const START_TEMPERATURE = 80;

function initialPositions(nodes: SeedNode[]): Record<string, XYPosition> {
  const clusters = new Map<number, SeedNode[]>();
  for (const node of nodes) {
    const members = clusters.get(node.community) ?? [];
    members.push(node);
    clusters.set(node.community, members);
  }

  const positions: Record<string, XYPosition> = {};
  for (const [community, members] of clusters) {
    const centreAngle = (community / COMMUNITY_COUNT) * Math.PI * 2 - Math.PI / 2;
    const cx = Math.cos(centreAngle) * CLUSTER_RING;
    const cy = Math.sin(centreAngle) * CLUSTER_RING;
    members.forEach((member, index) => {
      const angle = (index / members.length) * Math.PI * 2 - Math.PI / 2;
      positions[member.id] = {
        x: cx + Math.cos(angle) * MEMBER_RING,
        y: cy + Math.sin(angle) * MEMBER_RING,
      };
    });
  }
  return positions;
}

/**
 * Fruchterman-Reingold refinement on top of the community placement.
 *
 * Clusters alone leave every cross-cluster relation cutting across the middle of
 * the canvas, which reads as a hairball. Pulling connected concepts together
 * shortens those relations and puts the program hub in the centre, where its
 * degree deserves to be.
 */
function forceLayout(
  nodes: SeedNode[],
  edges: SeedEdge[],
  start: Record<string, XYPosition>
): Record<string, XYPosition> {
  const positions: Record<string, XYPosition> = {};
  for (const node of nodes) positions[node.id] = { ...start[node.id] };

  let temperature = START_TEMPERATURE;
  for (let iteration = 0; iteration < FORCE_ITERATIONS; iteration += 1) {
    const disp: Record<string, XYPosition> = {};
    for (const node of nodes) disp[node.id] = { x: 0, y: 0 };

    // Repulsion between every pair.
    for (let i = 0; i < nodes.length; i += 1) {
      for (let j = i + 1; j < nodes.length; j += 1) {
        const a = positions[nodes[i].id];
        const b = positions[nodes[j].id];
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const distance = Math.max(0.01, Math.hypot(dx, dy));
        const force = (IDEAL_EDGE_LENGTH * IDEAL_EDGE_LENGTH) / distance;
        disp[nodes[i].id].x += (dx / distance) * force;
        disp[nodes[i].id].y += (dy / distance) * force;
        disp[nodes[j].id].x -= (dx / distance) * force;
        disp[nodes[j].id].y -= (dy / distance) * force;
      }
    }

    // Attraction along relations.
    for (const edge of edges) {
      const a = positions[edge.source];
      const b = positions[edge.target];
      if (!a || !b) continue;
      const dx = a.x - b.x;
      const dy = a.y - b.y;
      const distance = Math.max(0.01, Math.hypot(dx, dy));
      const force = (distance * distance) / IDEAL_EDGE_LENGTH;
      disp[edge.source].x -= (dx / distance) * force;
      disp[edge.source].y -= (dy / distance) * force;
      disp[edge.target].x += (dx / distance) * force;
      disp[edge.target].y += (dy / distance) * force;
    }

    // Displace, capped by the cooling temperature; a light pull keeps the graph
    // from drifting off-centre.
    for (const node of nodes) {
      const vector = disp[node.id];
      const magnitude = Math.max(0.01, Math.hypot(vector.x, vector.y));
      const step = Math.min(magnitude, temperature);
      positions[node.id].x += (vector.x / magnitude) * step - positions[node.id].x * 0.004;
      positions[node.id].y += (vector.y / magnitude) * step - positions[node.id].y * 0.004;
    }

    temperature *= 0.985;
  }

  return positions;
}

/** Push overlapping nodes apart so the seeded graph reads cleanly at rest. */
function relax(nodes: SeedNode[], start: Record<string, XYPosition>): Record<string, XYPosition> {
  const positions: Record<string, XYPosition> = {};
  for (const node of nodes) positions[node.id] = { ...start[node.id] };

  for (let iteration = 0; iteration < 240; iteration += 1) {
    let moved = false;
    for (let i = 0; i < nodes.length; i += 1) {
      for (let j = i + 1; j < nodes.length; j += 1) {
        const a = positions[nodes[i].id];
        const b = positions[nodes[j].id];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let distance = Math.hypot(dx, dy);
        if (distance >= MIN_NODE_DISTANCE) continue;
        if (distance < 0.001) {
          dx = 1;
          dy = 0;
          distance = 1;
        }
        const push = (MIN_NODE_DISTANCE - distance) / 2;
        const ux = (dx / distance) * push;
        const uy = (dy / distance) * push;
        a.x -= ux;
        a.y -= uy;
        b.x += ux;
        b.y += uy;
        moved = true;
      }
    }
    if (!moved) break;
  }

  for (const node of nodes) {
    positions[node.id] = {
      x: Math.round(positions[node.id].x),
      y: Math.round(positions[node.id].y),
    };
  }
  return positions;
}

export function demoPositions(): Record<string, XYPosition> {
  return relax(NODES, forceLayout(NODES, EDGES, initialPositions(NODES)));
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

export const DEMO_SESSION_NAME = "Apollo Program";

/**
 * A fresh copy of the demo scene. Callers own the result: mutating it never
 * changes the next visitor's (or the next test's) seed.
 */
export function buildDemoScene(): PersistedScene {
  const nodes: KnowledgeNode[] = NODES.map(({ id, label, kind }) => ({ id, label, kind }));
  const edges: KnowledgeEdge[] = EDGES.map((edge, index) => ({
    id: `demo-e${index + 1}`,
    source: edge.source,
    target: edge.target,
    relation: edge.relation,
    confidence: edge.confidence,
    evidence: edge.evidence,
  }));

  const nodeSources: PersistedScene["nodeSources"] = {};
  for (const node of nodes) {
    nodeSources[node.id] = { note: DEMO_NOTE, createdAt: SEEDED_AT };
  }
  const edgeSources: PersistedScene["edgeSources"] = {};
  for (const edge of edges) {
    edgeSources[edge.id] = { note: DEMO_NOTE, createdAt: SEEDED_AT };
  }

  return {
    version: 2,
    nodes,
    edges,
    positions: demoPositions(),
    viewConfig: { type: "default", semanticZoom: "entity" },
    // Communities are detected from the graph at runtime and named by the API;
    // seeding authored labels would key them by index and mislabel real groups.
    communityLabels: {},
    nodeSources,
    edgeSources,
    savedAt: SEEDED_AT,
  };
}
