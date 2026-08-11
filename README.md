# Weave

Weave is an AI-powered knowledge graph application. You type plain-language notes and Weave automatically extracts concepts and relationships, weaving them into a living, adaptive knowledge graph — no manual node creation or connector drawing.

<img width="1853" height="1095" alt="image" src="https://github.com/user-attachments/assets/b8486116-c859-4ff4-853c-ebd5be7717cf" />


## Features

- **Automatic graph construction** — enter a note, get nodes and edges. Existing concepts are recognized and reused, never duplicated.
- **Infinite canvas** — the graph grows with your knowledge (React Flow).
- **Interactive editing** — drag, rename (double-click), delete, and connect nodes manually.
- **Adaptive visualization (Iteration 2)**
  - **Importance** — hub nodes are visually prominent (degree centrality).
  - **Progressive disclosure** — click a node to focus its neighborhood; a breadcrumb returns you to the full graph.
  - **Semantic zoom** — zooming changes information density (overview → category → entity → detail), not just camera scale.
  - **Community detection** — label-propagation clustering colors edges between related nodes.
  - **AI community labeling** — the LLM names detected communities (with graceful fallback to "Group N").
  - **Layout intelligence** — a deterministic community-ring layout keeps important nodes central and related nodes close (re-layout button).
  - **Adaptive granularity** — high-degree nodes act as entry points ("+N" badge) and expand into deeper neighborhoods.
  - **Graph flavors** — one knowledge graph, many projections. The **Topic view** tints nodes by kind.
  - **AI suggestions** — an optional, advisory panel surfaces semantic groups, missing edges, and duplicates.

## Architecture

```
                          USER
                            |
                            v
                          NOTE
                            |
                            v
                  INGESTION PIPELINE
                            |
                            v
                     weave-api (stateless Rust)
                            |
                            v
                           LLM
                            |
                            v
                     GRAPH DELTA (new nodes + edges only)
                            |
                            v
                   KNOWLEDGE GRAPH (client-owned, Zustand)
                            |
                            v
                GRAPH INTELLIGENCE (importance, communities, layout)
                            |
                            v
                    VIEW / PROJECTION (default, topic, focus, zoom)
                            |
                            v
                     LAYOUT ENGINE
                            |
                            v
                          CANVAS (React Flow)
```

The knowledge graph is the source of truth and lives **in the browser**. The backend is a stateless API: it builds a prompt, calls an OpenAI-compatible LLM, validates/deduplicates the returned delta, and hands new nodes and edges back to the frontend. Views are reversible projections — changing how the graph is displayed never mutates the underlying knowledge.

### Stack

| Layer | Tech |
|---|---|
| Frontend | Next.js 16 (App Router), React 19, Zustand, React Flow (`@xyflow/react`), Tailwind CSS v4, Motion |
| Backend | Rust, Axum, Tokio, serde, reqwest (rustls) |
| LLM | OpenAI-compatible provider via `OPENCODE_BASE_URL` (no key → deterministic mock extractor) |

## Getting Started

Requirements: Node.js 20+, Rust (edition 2024).

### 1. Backend

```bash
cd backend
cp .env.example .env        # add OPENCODE_API_KEY (optional; unset = mock mode)
cargo run
# weave-api listening on http://localhost:3001
```

### 2. Frontend

```bash
npm install                 # from the repo root (npm workspaces)
npm run dev                 # Next.js on http://localhost:3000
```

Open http://localhost:3000/app and start typing notes, e.g.:

1. `Harry Potter`
2. `Harry's best friends are Ron and Hermione.`
3. `Harry studies at Hogwarts.`

### LLM configuration

The backend reads these environment variables (see `backend/.env.example`):

| Variable | Default | Description |
|---|---|---|
| `OPENCODE_BASE_URL` | `https://opencode.ai/zen/go/v1` | OpenAI-compatible endpoint |
| `OPENCODE_MODEL` | `deepseek-v4-flash` | Model used for extraction/organization |
| `OPENCODE_API_KEY` | *(unset)* | Unset → deterministic mock extractor |

## Docker

Build and run the backend container:

```bash
cd backend
docker build -t weave-api .
docker run -p 3001:3001 \
  -e OPENCODE_API_KEY=... \
  -e OPENCODE_MODEL=deepseek-v4-flash \
  weave-api
```

## API

All routes are under `http://localhost:3001`.

| Route | Method | Description |
|---|---|---|
| `/health` | GET | Liveness check |
| `/api/graph/ingest` | POST | Extract a delta (new nodes/edges) from a note |
| `/api/graph/organize` | POST | Advisory AI analysis (groups, missing edges, duplicates) |
| `/api/graph/label-community` | POST | Suggest a name for a detected community |
| `/api/graph/search` | POST | Semantic search over the graph |
| `/api/status` | GET | LLM provider/config status |

## Scripts

```bash
npm run build   # frontend production build
npm run lint    # frontend ESLint
cd backend && cargo test   # backend unit tests
```

## Project Layout

```
backend/                Rust API (axum) — extract, organize, llm, models
frontend/src/app/       Next.js pages (/ and /app canvas)
frontend/src/components/canvas/   CanvasApp, WeaveNode, CanvasHeader, InputDock, InsightsPanel
frontend/src/lib/       store (knowledge graph), api, graph-ops, graph-projection,
                        communities, layout, useSemanticZoom, useCommunityLabels
```
