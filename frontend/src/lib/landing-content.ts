/**
 * Content for the public site. Kept as data so the page stays readable and the
 * marketing numbers can be checked against the repository by a unit test.
 *
 * This module must stay dependency-free: it is imported by a Node test.
 */

export type Confidence = "EXTRACTED" | "INFERRED" | "AMBIGUOUS";

/** Categorical colors for graph communities. Data encoding, not UI accents. */
export const COMMUNITY_COLORS = ["#34d399", "#7dd3fc", "#f5b14c"] as const;

export type GraphLevel = 0 | 1 | 2 | 3;
export type ZoomLevel = "overview" | "category" | "entity" | "detail";

export interface GraphNodeSpec {
  id: string;
  label: string;
  kind?: string;
  x: number;
  y: number;
  importance?: number;
  degree?: number;
  community?: number;
  /** Lowest semantic zoom level at which this node is visible. */
  minLevel?: GraphLevel;
}

export interface GraphEdgeSpec {
  id: string;
  from: string;
  to: string;
  relation?: string;
  confidence?: Confidence;
  minLevel?: GraphLevel;
}

export interface GraphSpec {
  nodes: GraphNodeSpec[];
  edges: GraphEdgeSpec[];
  communities?: Record<number, string>;
}

// ---------------------------------------------------------------------------
// Hero spec bar
// ---------------------------------------------------------------------------

export interface SpecItem {
  value: string;
  label: string;
}

export const SPEC_BAR: SpecItem[] = [
  {
    value: "0",
    label: "Tokens spent on structured notes, parsed in-process",
  },
  {
    value: "2-hop",
    label: "Only the relevant subgraph reaches the model",
  },
  {
    value: "3",
    label: "Grounding classes on every edge",
  },
  {
    value: "4",
    label: "Export formats, generated in the browser",
  },
  {
    value: "18",
    label: "MCP memory tools for any agent",
  },
];

// ---------------------------------------------------------------------------
// How it works
// ---------------------------------------------------------------------------

export interface HowStep {
  step: string;
  title: string;
  body: string;
}

export const HOW_IT_WORKS: HowStep[] = [
  {
    step: "01",
    title: "Write",
    body: "Type a note the way you would say it. No node palette, no connector tool, no diagram to maintain.",
  },
  {
    step: "02",
    title: "Watch",
    body: "Weave finds the concepts and relations, reuses concepts you already have, and extends the graph in place.",
  },
  {
    step: "03",
    title: "Refine",
    body: "Drag, rename, connect, or delete anything it gets wrong. The graph stays yours, and every edit is local.",
  },
];

// ---------------------------------------------------------------------------
// Extraction examples
// ---------------------------------------------------------------------------

export interface ExtractionExample {
  id: "outline" | "prose";
  label: string;
  file: string;
  note: string;
  tier: "deterministic" | "llm";
  tokens: number;
  summary: string;
  evidence: string;
  confidence: Confidence;
  graph: GraphSpec;
}

const OUTLINE_NOTE = `# Ada Lovelace
- [[Analytical Engine]] :: programmed
- [[Charles Babbage]] :: corresponded with

# Charles Babbage
- [[Analytical Engine]] :: designed`;

const PROSE_NOTE = `Katherine Johnson calculated trajectories for the Mercury and Apollo programs. She worked at NASA with a team of mathematicians.`;

export const EXTRACTION_EXAMPLES: ExtractionExample[] = [
  {
    id: "outline",
    label: "Structured note",
    file: "ada-lovelace.md",
    note: OUTLINE_NOTE,
    tier: "deterministic",
    tokens: 0,
    summary:
      "A Markdown outline states its own graph, so weave-core parses it directly. Same input, same output, no provider call.",
    evidence: "- [[Analytical Engine]] :: programmed",
    confidence: "EXTRACTED",
    graph: {
      nodes: [
        { id: "ada", label: "Ada Lovelace", kind: "person", x: 210, y: 230 },
        {
          id: "engine",
          label: "Analytical Engine",
          kind: "machine",
          x: 500,
          y: 120,
        },
        {
          id: "babbage",
          label: "Charles Babbage",
          kind: "person",
          x: 500,
          y: 330,
        },
      ],
      edges: [
        {
          id: "e1",
          from: "ada",
          to: "engine",
          relation: "programmed",
          confidence: "EXTRACTED",
        },
        {
          id: "e2",
          from: "ada",
          to: "babbage",
          relation: "corresponded with",
          confidence: "EXTRACTED",
        },
        {
          id: "e3",
          from: "babbage",
          to: "engine",
          relation: "designed",
          confidence: "EXTRACTED",
        },
      ],
    },
  },
  {
    id: "prose",
    label: "Prose note",
    file: "katherine-johnson.txt",
    note: PROSE_NOTE,
    tier: "llm",
    tokens: 412,
    summary:
      "Prose does not state a graph, so the extractor sends a bounded subgraph and asks for a delta. Only the delta comes back.",
    evidence: "She worked at NASA with a team of mathematicians.",
    confidence: "INFERRED",
    graph: {
      nodes: [
        {
          id: "kj",
          label: "Katherine Johnson",
          kind: "person",
          x: 190,
          y: 230,
        },
        { id: "nasa", label: "NASA", kind: "organization", x: 390, y: 105 },
        {
          id: "mercury",
          label: "Mercury Program",
          kind: "program",
          x: 560,
          y: 210,
        },
        {
          id: "apollo",
          label: "Apollo Program",
          kind: "program",
          x: 520,
          y: 350,
        },
      ],
      edges: [
        {
          id: "p1",
          from: "kj",
          to: "mercury",
          relation: "calculated trajectories for",
          confidence: "EXTRACTED",
        },
        {
          id: "p2",
          from: "kj",
          to: "apollo",
          relation: "calculated trajectories for",
          confidence: "EXTRACTED",
        },
        {
          id: "p3",
          from: "kj",
          to: "nasa",
          relation: "worked at",
          confidence: "EXTRACTED",
        },
        {
          id: "p4",
          from: "nasa",
          to: "mercury",
          relation: "oversaw",
          confidence: "INFERRED",
        },
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Canvas and evidence
// ---------------------------------------------------------------------------

export const CANVAS_GRAPH: GraphSpec = {
  communities: {
    0: "Ada's circle",
    1: "The engine",
    2: "The paper",
  },
  nodes: [
    {
      id: "ada",
      label: "Ada Lovelace",
      kind: "person",
      x: 250,
      y: 210,
      importance: 0.96,
      degree: 5,
      community: 0,
      minLevel: 0,
    },
    {
      id: "engine",
      label: "Analytical Engine",
      kind: "machine",
      x: 520,
      y: 235,
      importance: 0.9,
      degree: 4,
      community: 1,
      minLevel: 0,
    },
    {
      id: "babbage",
      label: "Charles Babbage",
      kind: "person",
      x: 425,
      y: 105,
      importance: 0.78,
      degree: 3,
      community: 0,
      minLevel: 0,
    },
    {
      id: "notes",
      label: "Notes",
      kind: "document",
      x: 175,
      y: 88,
      importance: 0.6,
      degree: 2,
      community: 0,
      minLevel: 1,
    },
    {
      id: "algorithm",
      label: "Algorithm",
      kind: "concept",
      x: 625,
      y: 115,
      importance: 0.7,
      degree: 2,
      community: 1,
      minLevel: 1,
    },
    {
      id: "bernoulli",
      label: "Bernoulli numbers",
      kind: "concept",
      x: 655,
      y: 315,
      importance: 0.55,
      degree: 1,
      community: 1,
      minLevel: 2,
    },
    {
      id: "punched",
      label: "Punched cards",
      kind: "artifact",
      x: 470,
      y: 375,
      importance: 0.59,
      degree: 2,
      community: 1,
      minLevel: 2,
    },
    {
      id: "menabrea",
      label: "Menabrea",
      kind: "person",
      x: 320,
      y: 345,
      importance: 0.64,
      degree: 2,
      community: 2,
      minLevel: 1,
    },
    {
      id: "translation",
      label: "Translation",
      kind: "document",
      x: 115,
      y: 265,
      importance: 0.5,
      degree: 2,
      community: 2,
      minLevel: 2,
    },
  ],
  edges: [
    {
      id: "c1",
      from: "ada",
      to: "engine",
      relation: "programmed",
      confidence: "EXTRACTED",
      minLevel: 0,
    },
    {
      id: "c2",
      from: "ada",
      to: "babbage",
      relation: "corresponded with",
      confidence: "EXTRACTED",
      minLevel: 0,
    },
    {
      id: "c3",
      from: "babbage",
      to: "engine",
      relation: "designed",
      confidence: "EXTRACTED",
      minLevel: 0,
    },
    {
      id: "c4",
      from: "ada",
      to: "notes",
      relation: "wrote",
      confidence: "EXTRACTED",
      minLevel: 1,
    },
    {
      id: "c5",
      from: "notes",
      to: "algorithm",
      relation: "describes",
      confidence: "EXTRACTED",
      minLevel: 1,
    },
    {
      id: "c6",
      from: "algorithm",
      to: "bernoulli",
      relation: "computes",
      confidence: "EXTRACTED",
      minLevel: 2,
    },
    {
      id: "c7",
      from: "engine",
      to: "punched",
      relation: "used",
      confidence: "INFERRED",
      minLevel: 2,
    },
    {
      id: "c8",
      from: "ada",
      to: "translation",
      relation: "translated",
      confidence: "EXTRACTED",
      minLevel: 2,
    },
    {
      id: "c9",
      from: "translation",
      to: "menabrea",
      relation: "from",
      confidence: "EXTRACTED",
      minLevel: 1,
    },
    {
      id: "c10",
      from: "menabrea",
      to: "engine",
      relation: "wrote about",
      confidence: "EXTRACTED",
      minLevel: 1,
    },
    {
      id: "c11",
      from: "translation",
      to: "notes",
      relation: "expanded into",
      confidence: "AMBIGUOUS",
      minLevel: 2,
    },
  ],
};

export interface CanvasLevel {
  value: ZoomLevel;
  level: GraphLevel;
  label: string;
  title: string;
  body: string;
}

export const CANVAS_LEVELS: CanvasLevel[] = [
  {
    value: "overview",
    level: 0,
    label: "Overview",
    title: "Hubs first",
    body: "Only the most connected concepts appear, sized by degree. This is the shape of the whole map before any detail.",
  },
  {
    value: "category",
    level: 1,
    label: "Category",
    title: "Communities, not clutter",
    body: "Label propagation groups related concepts. The LLM names each group, with a plain fallback when it cannot.",
  },
  {
    value: "entity",
    level: 2,
    label: "Entity",
    title: "Every concept",
    body: "Entry points carry a connection count and expand into a neighborhood. Focus is reversible, never destructive.",
  },
  {
    value: "detail",
    level: 3,
    label: "Detail",
    title: "Every relation, and why",
    body: "Relation labels appear with their grounding class. Select an edge to see the note and sentence behind it.",
  },
];

export const CANVAS_EDGE = {
  from: "Ada Lovelace",
  to: "Analytical Engine",
  relation: "programmed",
  confidence: "EXTRACTED" as Confidence,
  source:
    "Ada Lovelace wrote the first algorithm intended for the Analytical Engine.",
  evidence:
    "The Analytical Engine weaves algebraic patterns just as the Jacquard loom weaves flowers and leaves.",
  createdAt: "12 Jan 2026, 09:14",
} as const;

// ---------------------------------------------------------------------------
// Retrieval
// ---------------------------------------------------------------------------

export interface RetrievalResult {
  nodeId: string;
  label: string;
  score: number;
  channel: "lexical" | "semantic" | "graph path";
  why: string;
}

export interface RetrievalQuery {
  id: "babbage" | "compute" | "path";
  label: string;
  query: string;
  results: RetrievalResult[];
}

export const RETRIEVAL_QUERIES: RetrievalQuery[] = [
  {
    id: "babbage",
    label: "who worked with Babbage?",
    query: "who worked with Babbage?",
    results: [
      {
        nodeId: "ada",
        label: "Ada Lovelace",
        score: 0.92,
        channel: "semantic",
        why: "co-occurs with Charles Babbage in three notes",
      },
      {
        nodeId: "babbage",
        label: "Charles Babbage",
        score: 0.88,
        channel: "lexical",
        why: "exact label match in the query",
      },
      {
        nodeId: "menabrea",
        label: "Menabrea",
        score: 0.61,
        channel: "graph path",
        why: "two hops away through the Analytical Engine",
      },
    ],
  },
  {
    id: "compute",
    label: "what did the engine compute?",
    query: "what did the engine compute?",
    results: [
      {
        nodeId: "bernoulli",
        label: "Bernoulli numbers",
        score: 0.89,
        channel: "semantic",
        why: "computed by the Algorithm that the engine executes",
      },
      {
        nodeId: "algorithm",
        label: "Algorithm",
        score: 0.84,
        channel: "lexical",
        why: "shares the term compute through its relation",
      },
      {
        nodeId: "engine",
        label: "Analytical Engine",
        score: 0.77,
        channel: "graph path",
        why: "the anchor concept for this neighborhood",
      },
    ],
  },
  {
    id: "path",
    label: "how did Ada reach punched cards?",
    query: "how did Ada reach punched cards?",
    results: [
      {
        nodeId: "punched",
        label: "Punched cards",
        score: 0.83,
        channel: "graph path",
        why: "Ada Lovelace to Analytical Engine to Punched cards",
      },
      {
        nodeId: "engine",
        label: "Analytical Engine",
        score: 0.79,
        channel: "semantic",
        why: "the bridge concept in the path",
      },
      {
        nodeId: "translation",
        label: "Translation",
        score: 0.54,
        channel: "lexical",
        why: "mentions the engine in the source note",
      },
    ],
  },
];

// ---------------------------------------------------------------------------
// MCP memory
// ---------------------------------------------------------------------------

export interface McpTool {
  name: string;
  description: string;
}

export interface McpToolGroup {
  group: string;
  summary: string;
  tools: McpTool[];
}

export const MCP_TOOL_GROUPS: McpToolGroup[] = [
  {
    group: "Remember",
    summary: "Idempotent writes with extracted claims.",
    tools: [
      {
        name: "remember",
        description: "Store a note, extract claims, return a write receipt.",
      },
      {
        name: "add_file",
        description: "Store a file, extract text, chunk and ingest it.",
      },
    ],
  },
  {
    group: "Browse",
    summary: "Find what is already in memory.",
    tools: [
      { name: "list_notes", description: "Browse note summaries, newest first." },
      { name: "get_note", description: "Read a note and its graph linkage." },
      { name: "delete_note", description: "Delete a note by id." },
      { name: "search", description: "Full-text notes and keyword entities." },
    ],
  },
  {
    group: "Graph",
    summary: "Walk the server-owned structure.",
    tools: [
      { name: "get_node", description: "An entity and the relations touching it." },
      { name: "get_related", description: "BFS subgraph around an entity." },
      { name: "explain_node", description: "Degree and every relation, both directions." },
      { name: "path_between", description: "Shortest path, hop by hop." },
    ],
  },
  {
    group: "Recall",
    summary: "Retrieve the smallest useful context.",
    tools: [
      {
        name: "recall_memory",
        description: "Hybrid vector, FTS, graph, and claim retrieval.",
      },
      { name: "get_claim", description: "Evidence, source note, contradictions." },
      { name: "list_claims", description: "Claims about an entity, by status." },
    ],
  },
  {
    group: "Maintain",
    summary: "Correct, reindex, forget, and measure.",
    tools: [
      { name: "reindex_embeddings", description: "Re-embed rows on a new model." },
      { name: "forget_entity", description: "Audited hard delete of derived memory." },
      { name: "prune_notes", description: "Audited retention cleanup." },
      { name: "correct_claim", description: "Supersede a claim with a correction." },
      { name: "memory_stats", description: "Counts, statuses, coverage." },
    ],
  },
];

export const MCP_CONNECT_STDIO = `{
  "mcpServers": {
    "weave": {
      "command": "/path/to/weave-mcp",
      "env": {
        "WEAVE_MCP_DATABASE_URL":
          "postgres://weave:weave@localhost:5432/weave_mcp",
        "WEAVE_MCP_DATA_DIR": "/data/weave-mcp"
      }
    }
  }
}`;

export const MCP_CONNECT_HTTP = `{
  "mcpServers": {
    "weave": {
      "url": "http://127.0.0.1:8010/mcp"
    }
  }
}`;

export const MCP_RECALL_EXAMPLE = `recall_memory("who worked with Babbage?", top_k=4)

[claim] Ada Lovelace corresponded with Charles Babbage
  status: active  confidence: 0.94  modality: asserted
  evidence: "They corresponded for years about the Analytical Engine."
  source: note 8f2c, "Ada Lovelace"
  contradictions: none

[entity] Analytical Engine
  designed_by Charles Babbage
  programmed_by Ada Lovelace

merged from vector + full-text + graph expansion,
ranked with reciprocal-rank fusion.`;

export const MCP_CLAIM_EXAMPLE = `get_claim("clm_01J8Z6K4T3")

{
  "id": "clm_01J8Z6K4T3",
  "subject": "Ada Lovelace",
  "predicate": "programmed",
  "object": "Analytical Engine",
  "modality": "asserted",
  "status": "active",
  "confidence": 0.94,
  "evidence":
    "Ada wrote the first algorithm intended for the Analytical Engine.",
  "contradictions": []
}`;

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

export type ExportFormatId = "json" | "obsidian" | "graphml" | "cypher";

export interface ExportFormatOption {
  id: ExportFormatId;
  label: string;
  extension: string;
  mime: string;
  useFor: string;
}

export const EXPORT_FORMATS: ExportFormatOption[] = [
  {
    id: "json",
    label: "JSON snapshot",
    extension: ".json",
    mime: "application/json",
    useFor: "Backup, round-tripping, and archival. Provenance is inlined per node and edge.",
  },
  {
    id: "obsidian",
    label: "Obsidian vault",
    extension: ".zip",
    mime: "application/zip",
    useFor: "Read and edit the graph as linked notes with YAML frontmatter and wikilinks.",
  },
  {
    id: "graphml",
    label: "GraphML",
    extension: ".graphml",
    mime: "application/graphml+xml",
    useFor: "Open the graph in Gephi, yEd, NetworkX, or another graph tool.",
  },
  {
    id: "cypher",
    label: "Cypher",
    extension: ".cypher",
    mime: "text/plain",
    useFor: "Load the graph into Neo4j or Memgraph with MERGE statements.",
  },
];

// ---------------------------------------------------------------------------
// Platform trace
// ---------------------------------------------------------------------------

export interface PlatformStage {
  id: string;
  label: string;
  summary: string;
  detail: string;
  status: "ok" | "info";
  durationMs: number;
  code?: string;
  codeName?: string;
}

export const PLATFORM_STAGES: PlatformStage[] = [
  {
    id: "auth",
    label: "Authenticate",
    summary: "Session cookie verified server-side",
    detail:
      "The browser holds an opaque token in an HttpOnly, SameSite=Lax cookie. PostgreSQL stores only its hash. Google identity is keyed on the stable subject, never the email.",
    status: "ok",
    durationMs: 8,
  },
  {
    id: "permission",
    label: "Check permission",
    summary: "graph.ingest allowed for this role",
    detail:
      "Every route checks its permission against the role stored in PostgreSQL. Per-request permissions are resolved server-side; the UI never decides access.",
    status: "ok",
    durationMs: 3,
  },
  {
    id: "policy",
    label: "Resolve policy",
    summary: "role policy, user override, hard ceiling",
    detail:
      "Limits resolve from global to role to user override, with the most specific value winning. Every result is clamped to a hard ceiling so no policy can remove the safety net.",
    status: "ok",
    durationMs: 4,
    code: `{
  "user": "researcher@example.com",
  "role": "researcher",
  "inherited": { "requests_per_minute": 60 },
  "overrides": { "requests_per_minute": 120 },
  "effective": { "requests_per_minute": 120 }
}`,
    codeName: "effective policy",
  },
  {
    id: "rate_limit",
    label: "Rate limit",
    summary: "Redis Lua counters, blocked requests do not consume quota",
    detail:
      "Request, token, and concurrency counters are atomic Lua scripts in Redis. A blocked request never consumes quota, and the failure mode is explicit rather than silent.",
    status: "ok",
    durationMs: 2,
  },
  {
    id: "quota",
    label: "Token quota",
    summary: "budget checked before the model call",
    detail:
      "Token budgets are checked before any provider call. Exhaustion returns a standardized 429 with Retry-After, and the request is recorded as blocked, not metered.",
    status: "ok",
    durationMs: 2,
  },
  {
    id: "concurrency",
    label: "Concurrency slot",
    summary: "slot acquired and released on every path",
    detail:
      "A concurrency slot is held for the duration of the expensive work and released with an RAII guard, including on failure. A stalled provider cannot leak capacity.",
    status: "ok",
    durationMs: 1,
  },
  {
    id: "extract",
    label: "Extract concepts",
    summary: "deterministic parser first, bounded LLM second",
    detail:
      "A note that states its own graph is parsed in-process. Mostly-prose notes go to the model with a two-hop neighborhood capped at 64 nodes and 256 edges, so ingest cost does not scale with graph size.",
    status: "info",
    durationMs: 246,
    code: `{
  "format": "markdown",
  "structured_ratio": 0.86,
  "nodes": 3,
  "edges": 3,
  "evidence": "- [[Analytical Engine]] :: programmed",
  "provider_tokens": 0
}`,
    codeName: "deterministic extraction",
  },
  {
    id: "usage",
    label: "Record usage",
    summary: "provider tokens, latency, endpoint",
    detail:
      "Every LLM operation records provider-reported tokens, latency, and endpoint. Usage is what feeds quotas, analytics, and per-user cost visibility.",
    status: "ok",
    durationMs: 5,
  },
  {
    id: "response",
    label: "Respond",
    summary: "delta plus a request id",
    detail:
      "The response carries only the graph delta and a standardized error shape when something fails. Every response includes X-Request-ID so a trace can be followed end to end.",
    status: "ok",
    durationMs: 2,
  },
];

export const PLATFORM_POLICY_EXAMPLE = `{
  "user": "researcher@example.com",
  "role": "researcher",
  "inherited": { "requests_per_minute": 60 },
  "overrides": { "requests_per_minute": 120 },
  "effective": { "requests_per_minute": 120 }
}`;

export const PLATFORM_ERROR_EXAMPLE = `{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded",
    "request_id": "req_01J8Z6K4T3"
  }
}`;

// ---------------------------------------------------------------------------
// Architecture
// ---------------------------------------------------------------------------

export interface ArchitectureLane {
  label: string;
  title: string;
  detail: string;
  items: string[];
}

export const ARCHITECTURE_LANES: ArchitectureLane[] = [
  {
    label: "Browser",
    title: "Client-owned graph",
    detail:
      "React Flow, Zustand, and IndexedDB. The API never persists graph contents; views are reversible projections.",
    items: ["React Flow", "Zustand", "IndexedDB"],
  },
  {
    label: "Gateway",
    title: "Same-origin Next.js",
    detail:
      "The browser only talks to one origin. /api and /auth rewrite to weave-api, so the session cookie stays first-party.",
    items: ["Next.js", "/api rewrite", "HttpOnly cookie"],
  },
  {
    label: "API",
    title: "weave-api",
    detail:
      "Rust and Axum: OAuth, RBAC, Redis limits, usage, analytics, audit, extraction, organize, and search.",
    items: ["Rust", "Axum", "PostgreSQL", "Redis"],
  },
  {
    label: "Agents",
    title: "weave-mcp",
    detail:
      "Server-owned memory: notes, claims, pgvector, local ONNX embeddings, audit, and 18 tools over stdio or HTTP.",
    items: ["rmcp", "pgvector", "ONNX", "18 tools"],
  },
];

// ---------------------------------------------------------------------------
// Use cases
// ---------------------------------------------------------------------------

export interface UseCase {
  index: string;
  title: string;
  scenario: string;
  tag: string;
}

export const USE_CASES: UseCase[] = [
  {
    index: "01",
    title: "Study a novel",
    scenario: "Track characters, places, and factions, then ask who connects whom.",
    tag: "Literature",
  },
  {
    index: "02",
    title: "Map a codebase",
    scenario: "Turn services, modules, and ownership into one searchable map.",
    tag: "Engineering",
  },
  {
    index: "03",
    title: "Build a case file",
    scenario: "Keep people, claims, and the evidence line that supports each link.",
    tag: "Research",
  },
  {
    index: "04",
    title: "Trace a field",
    scenario: "Follow papers, methods, and findings until the gap is visible.",
    tag: "Science",
  },
  {
    index: "05",
    title: "Worldbuild a setting",
    scenario: "Keep cultures, histories, and conflicts consistent across notes.",
    tag: "Writing",
  },
  {
    index: "06",
    title: "Brief an agent",
    scenario: "Give an MCP agent claims with provenance, then recall only what matters.",
    tag: "AI memory",
  },
];

export const USE_CASE_DOMAINS = [
  "Literature",
  "Engineering",
  "Research",
  "Science",
  "Writing",
  "AI memory",
  "History",
  "Law",
  "Medicine",
  "Worldbuilding",
];

// ---------------------------------------------------------------------------
// Capability index
// ---------------------------------------------------------------------------

export type CapabilityCategory =
  | "Canvas"
  | "Extraction"
  | "Retrieval"
  | "Evidence"
  | "Portability"
  | "Platform"
  | "Agents"
  | "Interop";

export interface Capability {
  title: string;
  detail: string;
}

export interface CapabilityGroup {
  category: CapabilityCategory;
  items: Capability[];
}

export const CAPABILITY_GROUPS: CapabilityGroup[] = [
  {
    category: "Canvas",
    items: [
      { title: "Infinite canvas", detail: "Pan, zoom, box-select, and grow the graph in place." },
      { title: "Direct editing", detail: "Drag, rename, connect, and delete anything the model gets wrong." },
      { title: "Progressive disclosure", detail: "Focus a neighborhood, then return through a breadcrumb." },
      { title: "Semantic zoom", detail: "Overview, category, entity, and detail change density, not just scale." },
      { title: "Importance", detail: "Degree centrality makes hubs visually prominent." },
      { title: "Communities", detail: "Label propagation groups concepts; the LLM names each group with a fallback." },
      { title: "Adaptive granularity", detail: "High-degree nodes become entry points with a connection count." },
      { title: "Topic view and suggestions", detail: "Kind colors, groups, missing edges, duplicates, and isolated nodes." },
      { title: "Named sessions", detail: "Auto-saved project sessions with restore and reset." },
    ],
  },
  {
    category: "Extraction",
    items: [
      { title: "Plain-language ingest", detail: "One note becomes nodes and edges; no manual mapping." },
      { title: "Deterministic Markdown", detail: "Headings own links; relations keep the note's wording." },
      { title: "Deterministic JSON", detail: "Explicit nodes and edges, or nested relations." },
      { title: "Structure gate", detail: "At least 60 percent headings, bullets, or links, and at least one edge." },
      { title: "Duplicate reuse", detail: "Existing concepts are resolved and reused, never duplicated." },
      { title: "Bounded context", detail: "Two-hop neighborhood, 64 nodes, 256 edges, and a token budget." },
      { title: "Evidence lines", detail: "The source line is kept for every extracted edge." },
      { title: "Grounding classes", detail: "EXTRACTED, INFERRED, and AMBIGUOUS, with the supporting sentence." },
    ],
  },
  {
    category: "Retrieval",
    items: [
      { title: "Hybrid search", detail: "Lexical and embedding ranking merged into one result set." },
      { title: "Local ONNX embeddings", detail: "The API embeds the graph itself; no third-party embedding call." },
      { title: "Explainable similarity", detail: "Every result carries a score and a reason it matched." },
      { title: "Search to node", detail: "A ranked result jumps straight to the concept on the canvas." },
      { title: "Bounded organize", detail: "AI suggestions use a digest, never the whole graph." },
      { title: "Retrieval evaluation", detail: "A fixed corpus pins recall@k and MRR so ranking changes fail tests." },
    ],
  },
  {
    category: "Evidence",
    items: [
      { title: "Node provenance", detail: "Every node records the note and time that created it." },
      { title: "Edge provenance", detail: "Every relation records its originating note and time." },
      { title: "Inspector", detail: "One panel shows the label, kind, source, connections, and evidence." },
      { title: "Confidence shapes", detail: "Extracted edges are solid, inferred dashed, ambiguous amber." },
      { title: "Contradiction-safe memory", detail: "Server memory links contradicting claims instead of overwriting them." },
    ],
  },
  {
    category: "Portability",
    items: [
      { title: "JSON snapshot", detail: "Schema-versioned archive with provenance inlined." },
      { title: "Obsidian vault", detail: "One linked note per concept, with frontmatter and wikilinks." },
      { title: "GraphML", detail: "Edges carry relation, confidence, and evidence for external tools." },
      { title: "Cypher", detail: "MERGE statements for Neo4j and Memgraph." },
      { title: "Client-side export", detail: "Generated in the browser; nothing is uploaded." },
      { title: "Stable schema", detail: "The export version is independent of in-browser migrations." },
    ],
  },
  {
    category: "Platform",
    items: [
      { title: "Google OAuth", detail: "Authorization code flow with state and nonce verification." },
      { title: "Server-side sessions", detail: "Opaque token in an HttpOnly cookie; only its hash is stored." },
      { title: "Bootstrap admin", detail: "The first configured admin is promoted once, then the database owns it." },
      { title: "Role-based access", detail: "Permissions enforced on every route, not in the UI." },
      { title: "Rate limits", detail: "Requests, tokens, and concurrency from global to role to user." },
      { title: "Hard ceilings", detail: "No policy can raise a limit past the deployment ceiling." },
      { title: "Usage metering", detail: "Provider tokens, latency, and endpoint for every operation." },
      { title: "Analytics and audit", detail: "Product events plus an audited privileged-change log." },
      { title: "Pipeline tracer", detail: "Every ingest stage, prompt, raw response, and selected subgraph." },
      { title: "Standardized errors", detail: "One error shape and X-Request-ID on every response." },
    ],
  },
  {
    category: "Agents",
    items: [
      { title: "18 MCP tools", detail: "Notes, files, graph walks, claims, recall, correction, and forgetting." },
      { title: "Claims with evidence", detail: "Modality, confidence, status, and an evidence span per statement." },
      { title: "Selective verification", detail: "High-risk claims get a narrow structured review before commit." },
      { title: "Hybrid recall", detail: "Vector, full-text, graph expansion, and claims merged with RRF." },
      { title: "Idempotent writes", detail: "Identical content returns the existing note and receipt." },
      { title: "Audited mutations", detail: "Actor, action, before, and after on significant writes." },
      { title: "Correct and forget", detail: "Supersede a claim, hard-delete an entity, prune old notes." },
      { title: "Per-agent isolation", detail: "One database and data volume per agent instance." },
      { title: "stdio and HTTP", detail: "Run the binary or point an agent at the Streamable HTTP endpoint." },
    ],
  },
  {
    category: "Interop",
    items: [
      { title: "Common Ground adapter", detail: "The client-owned graph connects to a journaled collaborative protocol." },
      { title: "Human and agent turns", detail: "Manual edits and agent ops appear side by side in the journal." },
      { title: "Capability tokens", detail: "Ops are checked against a capability bound to the agent principal." },
      { title: "Approval and undo", detail: "Destructive ops wait for one-shot human approval; undo is a capability." },
      { title: "Public demo", detail: "A sign-in-free member session with a seeded example graph." },
    ],
  },
];

export const CAPABILITY_CATEGORIES: CapabilityCategory[] = [
  "Canvas",
  "Extraction",
  "Retrieval",
  "Evidence",
  "Portability",
  "Platform",
  "Agents",
  "Interop",
];
