# Weave

Weave is an AI-powered knowledge graph application. You type plain-language notes and Weave automatically extracts concepts and relationships, weaving them into a living, adaptive knowledge graph — no manual node creation or connector drawing.

<img width="1853" height="1095" alt="image" src="https://github.com/user-attachments/assets/b8486116-c859-4ff4-853c-ebd5be7717cf" />

## Features

- **Automatic graph construction** — enter a note, get nodes and edges. Existing concepts are recognized and reused, never duplicated.
- **Bounded LLM context (V1)** — only the subgraph relevant to a note reaches the extraction LLM (lexical + optional semantic anchors → 2-hop neighborhood, capped at 64 nodes / 256 edges / an estimated-token budget), so ingest cost doesn't scale with graph size.
- **Web semantic retrieval** — weave-api embeds the request graph on-the-fly (local ONNX): ingest grounding and `/api/graph/search` use hybrid lexical + embedding ranking with explainable similarity; a search box in the canvas header shows ranked results and jumps to nodes.
- **Graph provenance** — every node/edge records the note that created it (persisted per session); the node inspector shows its originating note and time.
- **Edge confidence** — every relationship is grounded against the note that produced it and tagged `EXTRACTED` (stated in the note), `INFERRED` (a deduction), or `AMBIGUOUS` (not supported by the note), with the supporting sentence. Inferred edges are drawn dashed, ambiguous ones amber, and the inspector lists each connection with its class and evidence.
- **Admin pipeline tracer** — an admin-only `/admin/pipeline` page shows every ingest stage end-to-end, including the exact prompt sent to the LLM, the raw response, and the subgraph (lexical + semantic anchors) that was selected.
- **Infinite canvas** — the graph grows with your knowledge (React Flow).
- **Interactive editing** — drag, rename (double-click), delete, and connect nodes manually.
- **Accounts & identity** — Google OAuth login, persistent users/roles, server-side sessions (HttpOnly cookie), bootstrap admin.
- **Authorization** — role-based permissions enforced server-side on every route.
- **Rate limits & quotas** — Redis-backed request/token/concurrency limits with global defaults, role policies, user overrides, and hard ceilings.
- **Usage metering** — every LLM operation records provider tokens, latency, and endpoint.
- **Analytics** — product/operational analytics events and an admin dashboard (users, roles, policies, usage, analytics, audit log).
- **AI assistant memory (MCP server)** — `weave-mcp` is a server-owned memory
  service for any MCP agent (Hermes, Claude Desktop, Codex): evidence-backed
  **claims** with provenance, hybrid lexical + semantic retrieval, selective
  deep verification of high-risk claims, idempotent writes, an audit log, and
  forgetting/correction lifecycle tools. (The web canvas and the agent memory
  service are separate workspaces for different use cases.)
- **Adaptive visualization (Iteration 2)**
  - **Importance** — hub nodes are visually prominent (degree centrality).
  - **Progressive disclosure** — click a node to focus its neighborhood; a breadcrumb returns you to the full graph.
  - **Semantic zoom** — zooming changes information density (overview → category → entity → detail), not just camera scale.
  - **Community detection** — label-propagation clustering colors edges between related nodes.
  - **AI community labeling** — the LLM names detected communities (with graceful fallback to "Group N").
  - **Layout intelligence** — a deterministic community-ring layout keeps important nodes central and related nodes close (re-layout button).
  - **Adaptive granularity** — high-degree nodes act as entry points ("+N" badge) and expand into deeper neighborhoods.
  - **Graph flavors** — one knowledge graph, many projections. The **Topic view** tints nodes by kind.
  - **AI suggestions** — an optional, advisory panel surfaces semantic groups, missing edges, and duplicates. Structural findings (hubs, bridges, isolated concepts, near-duplicate labels) are computed deterministically; the LLM only names groups and proposes missing edges, from a bounded digest rather than the whole graph.

## Common Ground adapter

Weave is the second Common Ground adapter. The knowledge graph stays client-owned; the Rust
journal owns op ids, revisions, capability checks, and audit entries.

- `frontend/src/lib/common-ground.ts` connects to `/common-ground/api/stream`, applies
  `graph.*` envelopes to the Zustand graph, computes inverses, and posts acks.
- Manual graph edits — rename, connect, delete — are recorded through
  `frontend/src/lib/graph-record.ts` with `cap_human`, so they appear as human turns in the
  Journal beside agent turns. Position-only drags and view changes are not journaled.
- The header badge shows the document suffix (`CG xxxxxxxx`); click it to open the Journal panel,
  which shows/copies the full document id, lists recent turns, approves `approval_required` ops
  with `cap_human`, and undoes a turn with `cap_undo`. An agent targets the full id.
- `frontend/src/app/common-ground/[...path]/route.ts` streams `/common-ground/*` to
  `COMMON_GROUND_URL` (default `http://127.0.0.1:8787`), including the SSE journal. Set
  `NEXT_PUBLIC_COMMON_GROUND_URL` to bypass the proxy.
- The `cap_graph` capability is bound to `weave-agent`. Create/update/view ops apply directly;
  node/edge deletes and merges require a one-shot human approval.

Ops: `node.create`, `node.update`, `node.delete`, `edge.create`, `edge.update`, `edge.delete`,
`merge`, `view.set`.

```sh
# With the Weave canvas open and its CG badge visible:
DOC_ID=weave_... node scripts/cg-smoke.mjs
```

The script mints a `cap_graph` token and submits one node.create envelope; the canvas should
show the new node after it acks.

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
                      weave-api (Rust)
                            |
            +---------------+---------------+
            |               |               |
      Authentication   Rate limiting   Usage / Analytics
      (Google OAuth)   (Redis, Lua)    (PostgreSQL)
            |               |               |
            +-------+-------+---------------+
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
              CANVAS (React Flow)
```

The knowledge graph is the source of truth and lives **in the browser**. The backend is an identity + platform-control API: it authenticates users, enforces permissions, rate limits LLM traffic, meters usage, records analytics, and runs the LLM extraction/analysis — but it never persists graph contents. Views are reversible projections — changing how the graph is displayed never mutates the underlying knowledge.

### Deployment topology

The frontend (Next.js) is a same-origin gateway: `next.config.ts` rewrites `/api/*` and `/auth/*` to the Rust backend, so the browser only ever talks to one origin. This makes `HttpOnly`/`SameSite=Lax` cookies work in development and keeps CORS locked to the frontend origin.

```
Browser → Next.js (:3000) ──/api, /auth──▶ weave-api (:3001) ──▶ PostgreSQL, Redis, LLM
```

### Stack

| Layer | Tech |
|---|---|
| Frontend | Next.js 16 (App Router), React 19, Zustand, React Flow (`@xyflow/react`), Tailwind CSS v4, Motion |
| Backend | Rust, Axum, Tokio, sqlx (PostgreSQL), redis, oauth2/openidconnect, serde, reqwest (rustls) |
| MCP server | Rust, `rmcp` (stdio / Streamable HTTP), sqlx + pgvector, fastembed (local ONNX embeddings, bundled in container builds) — `weave-mcp` |
| Shared | `crates/weave-core` — the OpenAI-compatible LLM client, text→graph extractor, and bounded-subgraph selector used by both `weave-api` and `weave-mcp` |
| State | PostgreSQL (users, roles, sessions, policies, usage, analytics, audit) · Redis (rate-limit/token/concurrency counters, OAuth state) |
| LLM | OpenAI-compatible provider via `OPENCODE_BASE_URL` (no key → deterministic mock extractor) |

## MCP Server (weave-mcp)

`weave-mcp` is a personal **knowledge / memory server** for AI assistants. Any
MCP agent (Hermes, Claude Desktop, Codex, …) connects over stdio and uses the
tools to store notes, files, and a **server-owned** knowledge graph — then
retrieve exactly the context it needs on demand, so the agent's own context
window stays small.

### Topology

```
 MCP agent (Hermes / Claude Desktop / Codex)
        │  MCP protocol over stdio or Streamable HTTP (JSON-RPC)
        ▼
 ┌────────────────────────── weave-mcp (mcp/) ─────────────────────────┐
 │  server.rs  (MemoryServer + 18 tools)                               │
 │      │             │               │                                │
 │  ingest.rs       recall.rs       graph.rs      (business logic)     │
 │  validate.rs  entity.rs  claims.rs  verify.rs  (V2/V4 semantics)    │
 │  retrieval.rs  embed.rs  audit.rs  (V3 hybrid retrieval / V5 audit) │
 │      │             │               │                                │
 │   store.rs ───── SQL ──▶ PostgreSQL "weave_mcp" (notes, entities,   │
 │   files.rs ─── blobs ──▶ data/       relations, claims, documents,  │
 │   embed.rs ── vectors ──▶ pgvector    provenance, audit, pgvector)  │
 │   summary.rs / weave-core ── LLM (shared with weave-api)            │
 └─────────────────────────────────────────────────────────────────────┘
```

Unlike the web app's **client-owned** graph (in the browser), `weave-mcp` keeps
a **server-owned** memory store in PostgreSQL — the persistent memory an agent can
read and write programmatically. Each agent runs its own instance/database
(isolation boundary). The `weave_mcp` database is created automatically on first
run; files are stored as hash-keyed blobs under `WEAVE_MCP_DATA_DIR`.

### What it stores

- **Notes/memories** — full text plus a compact LLM **summary** per note; long
  documents are chunked into `chunk` notes. Writes are **idempotent** (identical
  content returns the existing note) and **audited**.
- **Claims (the durable memory unit)** — every statement is validated
  deterministically, resolved (exact → alias → create), given an evidence span,
  a modality (asserted/negated/suggested/conditional), confidence, and status
  (active/contradicted/superseded/rejected/quarantined). Contradicting claims
  are linked and preserved — nothing is silently overwritten.
- **Selective deep verification** — high-risk claims (ambiguous resolution,
  contradiction, uncertain/comparative language, unsupported evidence) are
  reviewed by a narrow structured verifier before commit.
- **Files** — documents, PDFs, images, audio stored on disk with metadata;
  text is extracted for text-ish files and ingested into the graph.
- **Hybrid retrieval (V3)** — local ONNX embeddings (pgvector) + full-text FTS +
  graph expansion, merged with reciprocal-rank fusion into a compact context
  block; `recall_memory` can surface matching claims and flag contradictions.
  The ranking formula is pinned by an evaluation corpus (`mcp/tests/retrieval_eval.rs`):
  recall@k and MRR over fixed cases run with no database, so a weight change that
  makes retrieval worse fails the test suite.
- **Audit log** — every significant mutation records actor, action, and
  before/after values.

### Tools

| Tool | Purpose |
|---|---|
| `remember(text, kind?, tags?, agent_id?)` | Store a note/memory; idempotent; extract + validate + verify claims; returns a write receipt with claim ids |
| `add_file(path, description?, agent_id?)` | Store a file on disk, extract text, chunk long text, ingest into the graph |
| `list_notes(limit?, tag?, kind?)` | Browse note summaries, newest first |
| `get_note(id)` | Full note content + the entities/relations it created |
| `delete_note(id)` | Delete a note |
| `search(query, limit?)` | Full-text notes + keyword entities |
| `get_node(label)` | An entity and the relations touching it |
| `get_related(label, depth?)` | BFS subgraph around an entity (up to depth 3) |
| `explain_node(label)` | One entity's degree and every relation touching it, both directions |
| `path_between(from, to, max_depth?)` | Shortest path between two entities, hop by hop, with a rendered chain |
| `recall_memory(query, top_k?, include_contradicted?)` | **Flagship**: hybrid retrieval (vector + FTS + graph + claims) → compact context block |
| `get_claim(id)` | One evidence-backed claim + its source note + contradictions |
| `list_claims(entity_label, status?, limit?)` | Claims about an entity, filtered by status |
| `reindex_embeddings(limit?)` | Re-embed rows whose vectors are missing or from an older model |
| `forget_entity(label)` | Permanently forget an entity + derived memory (audited hard delete) |
| `prune_notes(days, limit?)` | Audited retention cleanup of old notes |
| `correct_claim(id, predicate?, object_label?)` | Supersede a claim and commit the corrected version |
| `memory_stats` | Counts, claim statuses, verifier stats, embedding coverage |

These tools form the stable public contract of the memory service.

### How to use

```bash
docker compose up -d          # postgres (pgvector) + redis
cd mcp
cp .env.example .env          # optional — defaults match docker compose
cargo run                     # weave-mcp, ready on stdio
```

The default build uses a deterministic stub embedder so retrieval works offline;
build with `--features embedding` to enable local semantic vectors (fastembed,
ONNX model downloaded on first use). The containerized deployment builds with
`http,embedding` so semantic retrieval is enabled by default. Point any MCP
client at the binary, e.g. for Claude Desktop:

```json
{
  "mcpServers": {
    "weave": {
      "command": "/path/to/weave/mcp/target/debug/weave-mcp",
      "env": {
        "WEAVE_MCP_DATABASE_URL": "postgres://weave:weave@localhost:5432/weave_mcp",
        "WEAVE_MCP_DATA_DIR": "/path/to/weave/mcp/data"
      }
    }
  }
}
```

See [`mcp/README.md`](mcp/README.md) for full setup, schema, and tests.

### Containerized deployment (Streamable HTTP)

The full stack can run as containers on a shared Docker network (`weave_net`).
In this mode `weave-mcp` serves the **Streamable HTTP** MCP endpoint at `/mcp`
instead of stdio, so MCP clients do not need the binary on disk — any agent
container (or the host) just points its `url` at the endpoint:

```bash
docker compose -f docker-compose.infra.yml up -d --build
```

| Service | Container port | Host |
|---|---|---|
| postgres (pgvector) | 5432 | 127.0.0.1:5433 |
| redis | 6379 | — |
| weave-mcp (asuna) | 8010 | 0.0.0.0:8010 (`/mcp`) |
| weave-mcp-harshit | 8011 | 0.0.0.0:8011 (`/mcp`) |
| weave-api | 3001 | 0.0.0.0:5000 |

**Per-agent isolation.** Each agent gets its own `weave-mcp` instance with its
own database and data volume, so no agent can read another's notes/files. The
default stack ships two: `weave-mcp` (DB `weave_mcp`, volume `weave_mcp_data`)
and `weave-mcp-harshit` (DB `weave_mcp_harshit`, volume `weave_mcp_harshit_data`).
To add another agent, copy the `weave-mcp-harshit` service block in
`docker-compose.infra.yml` and give it a unique `container_name`, port, database
name, and data volume; the database is auto-created on first start.

Connecting agents:

- **Host-native agent** (Hermes gateway, Claude Desktop, …) — point at the
  published port of *its* instance:
  ```yaml
  mcp_servers:
    weave: { url: http://127.0.0.1:8010/mcp }
  ```
- **Agent running in a container** — join it to `weave_net` and use the service
  name of *its* instance (no host-IP dependency):
  ```bash
  docker network connect weave_net <agent-container>
  ```
  ```yaml
  mcp_servers:
    weave: { url: http://weave-mcp-harshit:8011/mcp }
  ```
  Then restart the agent's gateway (`hermes gateway restart`) so it re-runs MCP
  discovery; verify with `hermes mcp test weave` / `hermes mcp list`.

`weave-mcp` selects its transport via `WEAVE_MCP_TRANSPORT` (`stdio` default |
`http`) and binds to `WEAVE_MCP_HTTP_ADDR`. The container image is built with
the `http,embedding` features (Debian trixie base — the prebuilt ONNX runtime
needs glibc ≥ 2.38); the embedding model is cached under `HF_HOME=/data/hf` on
the data volume and falls back to the stub embedder if it cannot load.
`WEAVE_MCP_ALLOWED_HOSTS` optionally restricts accepted `Host` headers; unset
means the server answers any host so service-name and gateway-IP clients both
work.

## Getting Started

Requirements: Node.js 20+, Rust (edition 2024), Docker (for PostgreSQL + Redis).

### 1. Infrastructure

```bash
docker compose up -d          # postgres (pgvector) + redis
```

### 2. Backend

```bash
cd backend
cp .env.example .env          # fill in GOOGLE_CLIENT_ID/SECRET, OPENCODE_API_KEY
cargo run
# weave-api listening on http://localhost:3001
```

### 3. Frontend

```bash
npm install                   # from the repo root (npm workspaces)
npm run dev                   # Next.js on http://localhost:3000
```

Open http://localhost:3000/app. Without Google OAuth credentials set, use the
"Continue with a test account" button on the login page (enabled by
`AUTH_STUB=true` in `backend/.env`, dev/test only).

### 4. MCP server (optional)

```bash
cd mcp
cp .env.example .env          # optional — defaults match docker compose
cargo run                     # weave-mcp, ready on stdio
# build with --features embedding for local semantic vectors
```

Then connect any MCP agent (see [MCP Server](#mcp-server-weave-mcp)).

## Authentication

- Google Authorization Code flow via `/auth/google` → `/auth/google/callback`.
- OAuth `state` is a one-time value stored in Redis; the returned ID token's
  issuer, audience, and nonce are verified; login is keyed on the stable Google
  `sub`, never email.
- Sessions are server-side: the browser holds an opaque 256-bit token in an
  `HttpOnly`, `SameSite=Lax`, `Secure`-in-production cookie; PostgreSQL stores
  only its SHA-256 hash.
- **Bootstrap admin**: the first account whose email is in
  `BOOTSTRAP_ADMIN_EMAILS` (and only while no admin exists) is granted the
  admin role. After that, admin privileges come from the database.

Routes:

| Route | Description |
|---|---|
| `GET /auth/google` | Begin Google sign-in |
| `GET /auth/google/callback` | OAuth callback |
| `GET /auth/me` | Current session (`{authenticated, user}`) |
| `POST /auth/logout` | Revoke the current session |
| `POST /auth/logout-all` | Revoke every session for the user |
| `POST /auth/test/login` | Dev/test-only stub login (`AUTH_STUB=true`) |

## Rate limits & quotas

Effective limits resolve **global → role → user override** (more specific wins)
and are clamped to a global hard ceiling. Policies can be endpoint-specific
(`graph.ingest`, `graph.organize`, `graph.label_community`, `graph.search`).

Semantics:
- **Requests/min (hour/day)** — accepted requests in a fixed window. Blocked
  requests do not consume quota.
- **Tokens/day (month)** — total provider-reported tokens consumed; the quota
  is checked before the LLM call and blocks once exhausted.
- **Concurrent requests** — requests currently executing; a slot is released
  even on failure (RAII guard).

Counters are atomic Lua scripts in Redis (no non-atomic GET/calculate/SET).
When blocked the API returns `429` with `Retry-After` and a standardized error:

```json
{ "error": { "code": "rate_limit_exceeded", "message": "Rate limit exceeded", "request_id": "…" } }
```

**Redis failure behavior:** authentication relies on PostgreSQL and is
unaffected. For expensive graph/LLM endpoints the backend **fails closed**
(503) rather than silently allowing unlimited LLM traffic. Cheap
health/status endpoints never require Redis.

## Admin API

All admin routes require an `admin.*` permission (server-side enforced).

| Route | Method | Permission |
|---|---|---|
| `/api/admin/users` | GET | `admin.users.read` |
| `/api/admin/users/{id}` | GET/PATCH | `admin.users.read` / `admin.users.update` |
| `/api/admin/roles` | GET/POST | `admin.roles.read` / `admin.roles.update` |
| `/api/admin/roles/{id}` | PATCH/DELETE | `admin.roles.update` |
| `/api/admin/policies` | GET | `admin.policies.read` |
| `/api/admin/policies/roles/{id}` | POST | `admin.policies.update` |
| `/api/admin/policies/users/{id}` | POST | `admin.policies.update` |
| `/api/admin/analytics/overview` | GET | `admin.analytics.read` |
| `/api/admin/analytics/users/{id}` | GET | `admin.analytics.read` |
| `/api/admin/pipeline/ingest` | POST | `admin.pipeline.read` — run the ingest pipeline in observe-only mode with a full LLM trace |
| `/api/admin/audit` | GET | `admin.audit.read` |

Policy responses use an inheritance-friendly shape so the UI can show
`inherited` vs `override` without reverse-engineering the DB:

```json
{ "user": "…", "role": "researcher", "inherited": { "requests_per_minute": 60 }, "overrides": { "requests_per_minute": 120 }, "effective": { "requests_per_minute": 120 } }
```

Every privileged change is written to the audit log in the same transaction as
the change. Safety guards prevent removing/disabling the last active admin and
deleting the last admin role.

## Graph API

All graph endpoints require a session and the matching `graph.*` permission.

| Route | Method | Description |
|---|---|---|
| `/api/graph/ingest` | POST | Extract a delta (new nodes/edges) from a note — the extraction LLM receives only the relevant bounded subgraph |
| `/api/graph/organize` | POST | Advisory AI analysis (groups, missing edges, duplicates) |
| `/api/graph/label-community` | POST | Suggest a name for a detected community |
| `/api/graph/search` | POST | Semantic search over the graph |
| `/api/status` | GET | LLM provider/config status |

Every response carries `X-Request-ID`; every error uses the standardized
`{error: {code, message, request_id}}` shape. Error codes: `unauthorized`,
`forbidden`, `rate_limit_exceeded`, `quota_exceeded`, `invalid_request`,
`not_found`, `conflict`, `service_unavailable`, `internal_error`.

## Environment

Backend variables are documented in `backend/.env.example`. Key ones:

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` / `REDIS_URL` | — | Required durable/hot-path stores |
| `GOOGLE_CLIENT_ID/SECRET/REDIRECT_URI` | — | OAuth credentials |
| `FRONTEND_URL` | `http://localhost:3000` | Redirect target + default CORS origin |
| `SESSION_COOKIE_NAME` | `weave_session` | Cookie name (keep in sync with the frontend) |
| `SESSION_TTL_SECONDS` | `2592000` | Session lifetime |
| `BOOTSTRAP_ADMIN_EMAILS` | — | Emails granted admin on first login (only while no admin exists) |
| `AUTH_STUB` | `false` | Dev-only `/auth/test/login` |
| `HARD_CEILING_REQUESTS_PER_MINUTE` / `HARD_CEILING_TOKENS_PER_DAY` | `300` / `20000000` | Absolute safety ceiling |
| `RATE_LIMIT_DEFAULT_*` | `30` / `500000` | Fallback when no policy row resolves |

## Scripts

```bash
npm run build    # frontend production build
npm run lint     # frontend ESLint
cd backend && cargo test          # backend unit + integration tests
cd frontend && npx playwright test  # e2e (requires docker compose up -d)
cargo test --workspace            # backend + weave-core + mcp tests
```

## Project Layout

```
backend/                Rust API (axum) — auth, admin (incl. pipeline tracer), rate limiting, usage, analytics, organize
backend/migrations/     sqlx migrations (schema + seed data)
crates/weave-core/      shared library — LLM client, graph models, extractor, bounded-subgraph selector (used by backend + mcp)
mcp/                    weave-mcp — server-owned agent memory service (stdio / Streamable HTTP)
mcp/migrations/         sqlx migrations for the weave_mcp database (notes, entities, relations, claims, documents, provenance, embeddings, audit)
frontend/src/app/       Next.js pages (/ landing, /app canvas, /login, /admin/* incl. /admin/pipeline)
frontend/src/components/canvas/   CanvasApp, WeaveNode, CanvasHeader, InputDock, InsightsPanel
frontend/src/components/auth/     RequireAuth, RequireAdmin, UserMenu
frontend/src/lib/       store (knowledge graph), api (client), auth, graph-ops, graph-projection, communities, layout
frontend/src/proxy.ts   Next.js route protection for /app and /admin
```
