# Weave

Weave is an AI-powered knowledge graph application. You type plain-language notes and Weave automatically extracts concepts and relationships, weaving them into a living, adaptive knowledge graph — no manual node creation or connector drawing.

<img width="1853" height="1095" alt="image" src="https://github.com/user-attachments/assets/b8486116-c859-4ff4-853c-ebd5be7717cf" />

## Features

- **Automatic graph construction** — enter a note, get nodes and edges. Existing concepts are recognized and reused, never duplicated.
- **Infinite canvas** — the graph grows with your knowledge (React Flow).
- **Interactive editing** — drag, rename (double-click), delete, and connect nodes manually.
- **Accounts & identity** — Google OAuth login, persistent users/roles, server-side sessions (HttpOnly cookie), bootstrap admin.
- **Authorization** — role-based permissions enforced server-side on every route.
- **Rate limits & quotas** — Redis-backed request/token/concurrency limits with global defaults, role policies, user overrides, and hard ceilings.
- **Usage metering** — every LLM operation records provider tokens, latency, and endpoint.
- **Analytics** — product/operational analytics events and an admin dashboard (users, roles, policies, usage, analytics, audit log).
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
| State | PostgreSQL (users, roles, sessions, policies, usage, analytics, audit) · Redis (rate-limit/token/concurrency counters, OAuth state) |
| LLM | OpenAI-compatible provider via `OPENCODE_BASE_URL` (no key → deterministic mock extractor) |

## Getting Started

Requirements: Node.js 20+, Rust (edition 2024), Docker (for PostgreSQL + Redis).

### 1. Infrastructure

```bash
docker compose up -d          # postgres:16 + redis:7
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
| `/api/graph/ingest` | POST | Extract a delta (new nodes/edges) from a note |
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
```

## Project Layout

```
backend/                Rust API (axum) — auth, admin, rate limiting, usage, analytics, extract, organize, llm
backend/migrations/     sqlx migrations (schema + seed data)
frontend/src/app/       Next.js pages (/ landing, /app canvas, /login, /admin/*)
frontend/src/components/canvas/   CanvasApp, WeaveNode, CanvasHeader, InputDock, InsightsPanel
frontend/src/components/auth/     RequireAuth, RequireAdmin, UserMenu
frontend/src/lib/       store (knowledge graph), api (client), auth, graph-ops, graph-projection, communities, layout
frontend/src/proxy.ts   Next.js route protection for /app and /admin
```
