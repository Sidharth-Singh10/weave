# Iteration 2 Implementation Guide
## Adaptive Knowledge Graph Visualization

This document is an implementation handoff for an engineering agent working on the current knowledge-graph project.

Its purpose is to:
1. Understand the current architecture and its constraints.
2. Preserve the working behavior of the existing Iteration 1 implementation.
3. Establish the architecture needed for Iteration 2.
4. Implement Iteration 2 incrementally.
5. Ensure **every implementation step leaves the project in a working state**.

---

# 1. Product Context

The project is an AI-powered knowledge graph application.

The original product idea is that the user writes natural-language information and the application automatically converts that information into nodes and relationships. The user should focus on thinking and writing while the system handles graph construction.

The original proposal describes:
- an infinite canvas,
- a single text input,
- automatic graph generation,
- automatic node creation,
- automatic relationship detection,
- existing-node recognition,
- interactive graph editing,
- and an evolving graph.

The long-term vision is broader than a conventional mind-map: the graph should become an adaptive representation of the user's knowledge.

Iteration 2 focuses specifically on **visualization**.

Reference:
- Iteration 1: the original proposal describes automatic graph construction and the goal of reducing manual organization.
- Iteration 2: the target is an adaptive knowledge graph visualization system.

Do not implement future ideas merely because they appear in the original proposal. Treat Iteration 2 as the current target. Features beyond Iteration 2 are optional future directions.

---

# 2. Current Implementation: Iteration 1

## 2.1 Core architecture

The current architecture follows a:

> Fat Client + Stateless Server

model.

### Frontend

The frontend owns the graph state.

Expected responsibilities:
- React UI
- Zustand graph state
- canvas rendering
- graph merging
- graph layout
- user interaction

The frontend is the source of truth for nodes and edges.

### Backend

The backend is `weave-api`, written in Rust.

The backend is stateless.

It does not persist the graph database.

Its current role is primarily:
1. receive an ingestion request,
2. construct the LLM prompt,
3. call an OpenAI-compatible LLM,
4. validate/deduplicate the returned graph delta,
5. return the delta to the frontend.

---

# 3. Current Ingestion Flow

The current flow is approximately:

```text
User enters text
      |
      v
Frontend gathers text + current graph
      |
      v
IngestRequest
      |
      v
weave-api
      |
      v
Prompt construction
      |
      v
LLM
      |
      v
New nodes + edges
      |
      v
Backend deduplication / safety checks
      |
      v
Frontend
      |
      v
Merge into Zustand
      |
      v
Layout
      |
      v
Canvas
```

The important existing behavior is:

> The LLM returns a graph delta rather than replacing the whole graph.

The backend also performs deduplication against the graph context supplied by the frontend.

---

# 4. Current Scaling Problem

The current ingestion implementation sends the entire graph to the backend/LLM for each ingestion operation.

Conceptually:

```text
New note
+
ALL nodes
+
ALL edges
        |
        v
       LLM
```

This creates three problems:

### 4.1 Payload growth

The JSON request grows with the number of nodes and edges.

### 4.2 LLM context growth

The prompt becomes increasingly large as the graph grows.

### 4.3 Entity resolution quality

The LLM has to search an increasingly large list of existing entities to determine whether a new entity already exists.

This can lead to:
- higher latency,
- higher token usage,
- more difficult entity matching,
- duplicated nodes,
- larger prompts,
- poorer reliability.

---

# 5. Important Architectural Constraint

Do NOT solve this by turning `weave-api` into a stateful graph server.

The frontend should continue to own the knowledge graph.

Instead, reduce the amount of graph context sent to the LLM.

The desired direction is:

```text
Frontend owns complete graph
          |
          v
Local graph retrieval
          |
          v
Relevant candidates
          |
          v
Small context
          |
          v
weave-api
          |
          v
LLM
```

The LLM should understand new information and resolve it against a small set of relevant candidates.

It should NOT act as the graph database.

---

# 6. Iteration 2 Goal

Iteration 2 changes the main focus from:

> Automatically constructing a graph

to:

> Automatically adapting how the graph is presented.

The Iteration 2 specification identifies these visualization capabilities:

1. Adaptive graph granularity
2. Multiple graph flavors
3. Importance-based node visualization
4. AI-assisted graph organization/layout
5. Semantic zoom
6. Automatic community detection
7. Progressive information disclosure

The overall vision is an **Adaptive Knowledge Graph System** in which AI has two responsibilities:

1. Knowledge Construction
2. Knowledge Presentation

For this implementation phase, prioritize the architectural foundation and the visualization behavior. Do not attempt to build every advanced AI behavior at once.

---

# 7. Critical Architectural Decision

The system must distinguish between:

## Knowledge Graph

The actual knowledge.

```text
Nodes
Edges
Notes
Provenance
Metadata
```

and:

## Visualization / Render Graph

What is currently shown to the user.

```text
Visible nodes
Visible edges
Groups
Collapsed nodes
Abstraction level
Positions
Node sizes
Visual metadata
```

These must NOT be treated as the same object.

The fundamental rule is:

> **The Knowledge Graph describes what exists. The View describes what the user currently sees.**

A visualization operation should generally not mutate the underlying knowledge graph.

---

# 8. Target Architecture

The desired architecture is:

```text
                         USER
                           |
                           v
                         NOTE
                           |
                           v
                  INGESTION PIPELINE
                           |
                           v
                     weave-api
                     (stateless)
                           |
                           v
                          LLM
                           |
                           v
                     GRAPH DELTA
                           |
                           v
                VALIDATION / RESOLUTION
                           |
                           v
                  KNOWLEDGE GRAPH
                           |
                           v
               GRAPH INTELLIGENCE LAYER
                           |
             +-------------+-------------+
             |             |             |
             v             v             v
         Importance    Communities    Hierarchy
             |             |             |
             +-------------+-------------+
                           |
                           v
                   VIEW / PROJECTION
                           |
                           v
                    LAYOUT ENGINE
                           |
                           v
                        CANVAS
```

The graph remains client-owned.

---

# 9. Target Data Model

## 9.1 Knowledge Node

Move toward stable IDs.

Do not use labels as primary identity.

Recommended conceptual structure:

```typescript
type KnowledgeNode = {
  id: string;
  label: string;
  kind: NodeKind;

  aliases?: string[];

  metadata?: Record<string, unknown>;

  provenance?: Provenance[];
};
```

Example:

```json
{
  "id": "node_hogwarts",
  "label": "Hogwarts",
  "kind": "org"
}
```

## 9.2 Knowledge Edge

Edges should reference node IDs.

```typescript
type KnowledgeEdge = {
  id: string;

  source: string;
  target: string;

  relation: string;

  provenance?: Provenance[];
};
```

Do NOT make:

```json
{
  "source_label": "Harry Potter",
  "target_label": "Hogwarts"
}
```

the canonical internal representation.

Labels are display values, not identity.

---

# 10. Target View Model

Introduce a visualization abstraction.

Conceptually:

```typescript
type GraphView = {
  id: string;

  type: ViewType;

  visibleNodes: string[];
  visibleEdges: string[];

  collapsedGroups?: string[];

  zoomLevel: number;

  layout: LayoutState;
};
```

Possible future/current view types:

```typescript
type ViewType =
  | "default"
  | "character"
  | "timeline"
  | "location"
  | "topic";
```

Do not implement all view types immediately.

The first working view should simply be:

```text
default
```

---

# 11. Render Model

The renderer should not need to understand the entire knowledge graph.

Conceptually:

```typescript
type RenderNode = {
  id: string;

  sourceNodeId: string;

  label: string;

  x: number;
  y: number;

  size: number;

  importance: number;

  depth: number;

  collapsed?: boolean;
};
```

and:

```typescript
type RenderEdge = {
  id: string;

  source: string;
  target: string;

  relation?: string;

  visible: boolean;
};
```

Exact fields may be adapted to the existing frontend/canvas library.

Do not blindly create duplicate state if the current renderer already has equivalent structures. Reuse existing structures where appropriate.

---

# 12. Implementation Strategy

The implementation must be incremental.

## NON-NEGOTIABLE RULE

> **Every step below must finish with the application/build/tests in a working state.**

Do not perform a large architectural rewrite followed by a broken intermediate state.

After each step:
1. compile,
2. run relevant tests,
3. run the application,
4. manually verify the existing ingestion flow,
5. only then continue.

If a step causes regressions, fix them before starting the next step.

---

# STEP 0 — Establish a Baseline

## Goal

Understand and record the existing working behavior before modifying architecture.

## Tasks

Inspect:
- frontend package structure,
- Zustand stores,
- graph types,
- ingestion request/response types,
- Rust API routes,
- LLM prompt construction,
- deduplication,
- current layout code,
- canvas implementation.

Identify:
- where nodes are stored,
- where edges are stored,
- how node IDs are currently generated,
- whether edges already use IDs or labels,
- how layout is calculated,
- where graph merging occurs.

Do not modify behavior yet.

## Working-state requirement

The application must work exactly as before.

Verify:
- application starts,
- a note can be entered,
- nodes are generated,
- edges are generated,
- existing nodes are reused,
- graph renders correctly.

---

# STEP 1 — Introduce Stable Node and Edge IDs

## Goal

Make node/edge identity independent of labels.

## Tasks

Introduce stable IDs if they do not already exist.

Example:

```typescript
type NodeId = string;
type EdgeId = string;
```

Nodes:

```typescript
{
  id,
  label,
  kind
}
```

Edges:

```typescript
{
  id,
  source,
  target,
  relation
}
```

Update all internal graph operations to use IDs.

Labels should remain available for:
- display,
- search,
- LLM context,
- user editing.

Do not use labels as canonical references internally.

## Compatibility

If the current LLM output uses labels, create a translation layer.

Do NOT require the entire LLM system to be rewritten in one step.

## Working state

Existing examples must still work:

```text
Harry Potter
  |
studies at
  |
Hogwarts
```

Renaming a node must not break its edges.

---

# STEP 2 — Extract Graph Operations into a Graph Transaction Layer

## Goal

Centralize graph mutation.

Create a single mechanism conceptually similar to:

```typescript
applyGraphDelta(graph, delta)
```

The operation should support at least:

```text
create node
create edge
update node
update edge if necessary
```

The LLM must not directly mutate Zustand.

The flow becomes:

```text
LLM response
    |
    v
GraphDelta
    |
    v
validation
    |
    v
applyGraphDelta()
    |
    v
Zustand
```

## Working state

Existing ingestion must still create exactly the same visible result.

This step is successful only if graph mutations are centralized without breaking the existing UI.

---

# STEP 3 — Separate Knowledge State From Render State

## Goal

Introduce the architecture required by Iteration 2.

The knowledge graph remains the source of truth.

The canvas receives a projection/render representation.

Desired flow:

```text
Knowledge Graph
      |
      v
Default Graph Projection
      |
      v
Render Graph
      |
      v
Canvas
```

Initially the projection should be almost identity:

```text
all knowledge nodes
        ↓
all visible
```

and:

```text
all knowledge edges
        ↓
all visible
```

Do NOT implement semantic zoom, communities, or flavors yet.

## Working state

The canvas should look and behave essentially the same as before.

This is a critical milestone.

If the visualization is worse after this step, stop and fix it.

---

# STEP 4 — Introduce a View/Projection Pipeline

## Goal

Create a clean pipeline for transforming knowledge into visualization.

Conceptually:

```typescript
createGraphView(
  knowledgeGraph,
  viewConfig
)
```

The result should determine:
- visible nodes,
- visible edges,
- grouping,
- abstraction,
- display metadata.

Start with:

```text
DefaultView
```

No advanced filtering.

## Working state

The default projection should reproduce the current graph.

This establishes the extension point for Iteration 2.

---

# STEP 5 — Add Importance Scoring

## Goal

Make the graph visually communicate that some concepts matter more than others.

Do not start with an LLM.

Create a deterministic importance score.

Possible initial inputs:
- node degree,
- number of mentions if available,
- graph centrality if practical,
- optionally user interaction count if already available.

Start simple.

Example:

```text
importance =
  normalizedDegree
```

Then evolve later.

The score should be metadata for visualization, not a mutation of the knowledge graph.

## Visualization

Use importance to influence something conservative, such as:
- node size,
- label size,
- visual prominence.

Avoid extreme scaling.

## Working state

The graph still:
- renders,
- pans,
- zooms,
- accepts new notes,
- creates nodes,
- creates edges.

Only visual prominence changes.

---

# STEP 6 — Add Progressive Disclosure

## Goal

Reduce visual overload by showing less information when appropriate.

Implement this deterministically first.

For example, a selected/high-level node can expose its immediate neighborhood while unrelated details remain hidden.

Start with a simple interaction:

```text
Default:
show important/high-level nodes

Select node:
show its relevant neighborhood
```

Do NOT yet implement full semantic zoom.

## Working state

The user must always be able to:
- understand the current context,
- select a node,
- expand it,
- return to the previous view.

Do not permanently remove knowledge from the graph.

Only change visibility in the projection.

---

# STEP 7 — Add Semantic Zoom

## Goal

Make zoom change information density, not only camera scale.

Define a small number of semantic levels.

Example:

```typescript
type SemanticZoomLevel =
  | "overview"
  | "category"
  | "entity"
  | "detail";
```

Initial behavior can be simple.

Example:

### Overview

Show:
- highly important nodes,
- major groups.

### Category

Show:
- category/group nodes,
- important entities.

### Entity

Show:
- individual entities and relationships.

### Detail

Show:
- dense local relationships.

The exact rules should be deterministic initially.

Do not require an LLM call every time the user moves the mouse wheel.

## Working state

Zooming should remain smooth.

The canvas must not become unusable due to excessive LLM requests.

---

# STEP 8 — Add Community Detection

## Goal

Detect naturally connected groups.

Start with graph algorithms.

Possible implementation approaches can be chosen based on the existing stack, but the principle is:

```text
Knowledge Graph
      |
      v
Community Detection
      |
      v
Clusters
```

Example:

```text
Harry
Ron
Hermione
Neville
      ↓
cluster_1
```

At first, clusters can simply be visual groups.

Do not require the AI to invent labels yet.

## Working state

Clusters should:
- render correctly,
- be collapsible if implemented,
- not alter underlying node/edge data.

---

# STEP 9 — Introduce AI-Assisted Group Naming

## Goal

Use the LLM for semantic interpretation rather than geometry.

After deterministic community detection produces a cluster:

```text
Harry
Ron
Hermione
Neville
```

the AI can suggest:

```text
Students
```

The LLM should receive only the cluster context, not the entire graph.

Example request:

```json
{
  "nodes": [
    "Harry Potter",
    "Ron Weasley",
    "Hermione Granger",
    "Neville Longbottom"
  ]
}
```

Possible response:

```json
{
  "label": "Students"
}
```

This is optional if the cluster is already understandable.

## Working state

AI failures must not break visualization.

If the AI request fails:

```text
cluster still works
```

It can temporarily display:

```text
Group
```

or remain unlabeled.

---

# STEP 10 — Introduce Deterministic Layout Intelligence

## Goal

Improve layout using graph structure.

Do NOT ask the LLM for x/y coordinates.

The layout engine should handle:
- node positions,
- collision avoidance,
- spacing,
- group placement,
- edge routing,
- stable positions.

Semantic information can influence layout constraints.

For example:

```text
important nodes → central
same community → closer
different communities → farther apart
```

The actual coordinates remain the responsibility of the layout engine.

## Working state

Adding a new node must not completely destroy the user's existing layout.

Prefer incremental layout where possible.

---

# STEP 11 — Add the First Adaptive Granularity Behavior

## Goal

Implement the first real version of the Iteration 2 "large node becomes an entry point to a subgraph" concept.

Do NOT physically split the knowledge graph.

Instead:

```text
Knowledge Graph
       |
       v
View Projection
       |
       v
Hermione node
   "+45 concepts"
```

Selecting Hermione changes the view:

```text
Hermione-centered projection
```

The same underlying nodes and edges remain in the knowledge graph.

## Working state

Navigation must provide a clear way to return:

```text
Main Graph
   >
Hermione
```

The user should never lose the global graph.

---

# STEP 12 — Add Graph Flavors

Only after the default visualization is stable should graph flavors be introduced.

The Iteration 2 specification proposes:
- Character Graph
- Timeline Graph
- Location Graph
- Topic Graph

Do not create separate datasets.

Instead:

```text
One Knowledge Graph
        |
        +--> Character View
        +--> Timeline View
        +--> Location View
        +--> Topic View
```

Each flavor is a projection.

## First flavor

Implement only one flavor initially.

Recommended first choice:

```text
Topic View
```

or the flavor most compatible with the existing node/edge data.

The choice should be based on what metadata the current implementation actually has.

## Working state

Switching views must never duplicate or mutate the knowledge graph.

Returning to the default view must recover the previous knowledge representation.

---

# STEP 13 — Make AI Presentation Analysis an Optional Layer

Only after deterministic visualization works should AI presentation analysis be introduced.

AI can help with:

```text
semantic grouping
group labels
hierarchy suggestions
importance hints
relationship interpretation
view recommendations
```

AI should NOT continuously control:
- raw coordinates,
- every zoom event,
- every node's rendering,
- every frame,
- every interaction.

The target architecture is:

```text
Algorithms:
"What structure exists?"

LLM:
"What does this structure mean?"

Visualization engine:
"How should it be rendered?"
```

---

# 13. Ingestion Architecture During Iteration 2

While implementing visualization, preserve the ingestion pipeline.

However, the scaling improvement should eventually become:

```text
New note
   |
   v
Mention detection
   |
   v
Local entity resolution
   |
   +--> exact match
   |
   +--> normalized/fuzzy match
   |
   +--> semantic retrieval if necessary
   |
   v
Relevant candidates
   |
   v
Relevant graph neighborhood
   |
   v
weave-api
   |
   v
LLM
   |
   v
GraphDelta
   |
   v
Validation
   |
   v
Knowledge Graph
```

Do not send the entire graph to the LLM when it is not necessary.

This can be implemented independently of visualization, but the final architecture should support it.

---

# 14. Graph Context Retrieval

Introduce a local retrieval layer.

Its purpose is:

> Given a new note, find the small portion of the graph that is likely relevant.

Possible sequence:

```text
New note
   |
   v
Extract candidate mentions
   |
   v
Exact matching
   |
   v
Normalized matching
   |
   v
Fuzzy matching
   |
   v
Semantic retrieval if necessary
   |
   v
Candidate nodes
   |
   v
Neighborhood expansion
   |
   v
LLM context
```

The LLM should normally receive a small candidate set rather than the complete graph.

This is especially important as Iteration 2 makes larger graphs useful.

---

# 15. Provenance

If practical in the current codebase, add provenance to knowledge nodes/edges.

Example:

```typescript
type Provenance = {
  sourceType: "note" | "user" | "import";
  sourceId?: string;
  text?: string;
};
```

This will later support:
- explaining why an edge exists,
- debugging AI extraction,
- user correction,
- reprocessing,
- source-aware visualization.

Do not let provenance become a blocker for the main visualization milestones.

---

# 16. Failure Handling

Every AI feature must degrade gracefully.

Examples:

### LLM unavailable

The existing graph remains usable.

### Community-label request fails

The community still exists.

### Importance calculation fails

Use default importance.

### Layout fails

Keep previous valid positions.

### View projection fails

Fall back to the default view.

### Retrieval fails

Fall back to the current ingestion behavior if necessary.

The application should never become unusable because an optional AI visualization feature failed.

---

# 17. Testing Requirements

Every implementation step must add or update tests where appropriate.

Minimum categories:

## Graph correctness

Test:
- node creation,
- node lookup,
- edge creation,
- node rename,
- stable IDs,
- graph delta application.

## Projection correctness

Test:
- all nodes visible in default view,
- filtering,
- collapsing,
- expansion,
- semantic zoom levels.

## Visualization intelligence

Test:
- importance scores,
- community detection,
- layout constraints.

## Regression

After every architectural change:

```text
Add note
  ↓
Graph updates
  ↓
Existing nodes are reused
  ↓
Edges are correct
  ↓
Canvas renders
```

must still work.

---

# 18. Definition of Done for Each Step

A step is NOT complete merely because the code compiles.

Each step is complete only when:

- implementation is complete,
- types are consistent,
- relevant tests pass,
- frontend builds,
- backend builds if backend was changed,
- the application starts,
- existing ingestion works,
- existing graph visualization works,
- no known regression is left unresolved.

If a migration requires temporary compatibility code, keep it until the next working state can safely remove it.

---

# 19. Recommended Implementation Order

Use exactly this high-level progression:

```text
0. Baseline
       ↓
1. Stable IDs
       ↓
2. Graph transaction layer
       ↓
3. Knowledge graph / render separation
       ↓
4. View / projection pipeline
       ↓
5. Importance
       ↓
6. Progressive disclosure
       ↓
7. Semantic zoom
       ↓
8. Community detection
       ↓
9. AI community labeling
       ↓
10. Deterministic layout intelligence
       ↓
11. Adaptive granularity
       ↓
12. First graph flavor
       ↓
13. Optional AI presentation intelligence
```

Do not skip directly to Steps 7–12.

The earlier steps establish the architecture that makes those features safe.

---

# 20. Explicit Non-Goals

Do NOT implement these unless specifically requested later:

- persistent server-side graph database,
- collaborative editing,
- voice input,
- PDF import,
- quiz generation,
- export to PDF/slides,
- real-time collaboration,
- full autonomous AI layout,
- arbitrary AI-generated x/y coordinates,
- multiple independent graph databases,
- a vector database solely because semantic retrieval might be useful later.

These belong to future expansion or are not required for the current Iteration 2 target.

---

# 21. Engineering Principles

## Principle 1 — One source of truth

The Knowledge Graph is the source of truth.

## Principle 2 — Views are projections

Visualization must be derived from knowledge.

## Principle 3 — AI is not the database

Use deterministic retrieval and graph algorithms wherever possible.

## Principle 4 — AI should handle semantics

Use the LLM for:
- extraction,
- interpretation,
- semantic grouping,
- naming,
- suggestions.

## Principle 5 — Algorithms should handle geometry

Use deterministic code for:
- positions,
- layout,
- collision,
- graph traversal,
- community structure,
- visibility.

## Principle 6 — Visualization must be reversible

Changing views must never destroy knowledge.

## Principle 7 — Every step must work

Never leave the repository in a knowingly broken intermediate state.

## Principle 8 — Prefer incremental migration

Adapt the existing implementation rather than rewriting the entire application.

---

# 22. Agent Workflow

Before modifying code:

1. Inspect the repository.
2. Identify frontend and backend entry points.
3. Identify current graph types.
4. Identify current Zustand state.
5. Identify current ingestion flow.
6. Identify current layout implementation.
7. Identify current canvas implementation.
8. Identify existing tests.

Then implement only one step at a time.

For each step:

```text
Inspect
  ↓
Plan small change
  ↓
Implement
  ↓
Build
  ↓
Test
  ↓
Run application
  ↓
Verify existing behavior
  ↓
Commit/checkpoint
  ↓
Next step
```

Do not batch several architectural steps into one unverified change.

---

# 23. Checkpoint Strategy

After each successful step, create a clear checkpoint.

Recommended checkpoint names:

```text
iteration2-step0-baseline
iteration2-step1-stable-ids
iteration2-step2-graph-transactions
iteration2-step3-knowledge-render-separation
iteration2-step4-view-projection
iteration2-step5-importance
iteration2-step6-progressive-disclosure
iteration2-step7-semantic-zoom
iteration2-step8-community-detection
iteration2-step9-ai-community-labeling
iteration2-step10-layout-intelligence
iteration2-step11-adaptive-granularity
iteration2-step12-first-graph-flavor
iteration2-step13-ai-presentation
```

Use the repository's existing Git conventions if they differ.

---

# 24. Final Target State

At the end of the Iteration 2 implementation, the application should conceptually work like this:

```text
                         USER
                           |
                           v
                         NOTE
                           |
                           v
                    KNOWLEDGE ENGINE
                           |
                           v
                    KNOWLEDGE GRAPH
                           |
             +-------------+-------------+
             |             |             |
             v             v             v
         Importance    Communities    Metadata
             |             |             |
             +-------------+-------------+
                           |
                           v
                     VIEW ENGINE
                           |
             +-------------+-------------+
             |             |             |
             v             v             v
         Visibility     Semantic      Grouping
         filtering        zoom
             |             |             |
             +-------------+-------------+
                           |
                           v
                    LAYOUT ENGINE
                           |
                           v
                         CANVAS
```

The same knowledge graph should be capable of producing different visual experiences.

For example:

```text
One knowledge graph
        |
        +--> Overview
        |
        +--> Character view
        |
        +--> Topic view
        |
        +--> Timeline view
        |
        +--> Hermione-focused view
```

without duplicating the underlying knowledge.

---

# 25. Final Instruction to the Agent

You are not being asked to build the entire future vision in one pass.

You are being asked to evolve the current working Iteration 1 implementation toward Iteration 2.

The highest-priority architectural distinction is:

```text
KNOWLEDGE GRAPH
      ≠
VISUALIZATION
```

Build the system so that:

```text
Knowledge Graph
      ↓
Graph Intelligence
      ↓
View / Projection
      ↓
Layout
      ↓
Canvas
```

is explicit in the codebase.

Most importantly:

> **After every implementation step, the application must remain runnable and the existing core behavior must continue to work.**

Prefer small, reversible changes over a large rewrite.

Do not implement future features simply because they are mentioned in the broader product vision.

Iteration 2 visualization is the current target.
