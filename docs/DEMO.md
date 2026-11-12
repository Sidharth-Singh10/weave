# Public demo deployment

Weave normally requires a Google account. The public demo is the same application
with one extra door: `POST /auth/demo/login` mints a fresh, non-admin `member`
session, and the landing page's primary call to action points at `/demo` instead of
`/app`. A reviewer opens the URL in a private window and lands on a populated,
interactive canvas.

Nothing about the demo changes how the real product authenticates. With the flag off,
`/auth/demo/login` returns 404 and the landing page's CTAs point at `/app` again.

## Topology

```
reviewer
  └─ https://weave.mutref.tech            Cloudflare DNS (CNAME, DNS-only)
       └─ Vercel: Next.js                project weave-frontend, root dir frontend
            └─ /api/*, /auth/*  ──▶  weave-api on the VPS (nginx vhost)
                                        └─ PostgreSQL + Redis (Docker)
```

The browser only ever talks to the Vercel origin. `next.config.ts` rewrites `/api/*`
and `/auth/*` to `WEAVE_API_URL`, so the session cookie is issued for the frontend
origin and `proxy.ts` can see it.

`WEAVE_API_URL` is read when `next.config.ts` is evaluated, i.e. at **build** time.
Changing it requires a new deployment.

## Configuration

### Vercel project

| Variable | Value | Notes |
|---|---|---|
| `WEAVE_API_URL` | `https://<api-host>` | Already set; leave unchanged |
| `NEXT_PUBLIC_DEMO_MODE` | `true` | Build-time: redeploy after adding it |
| `NEXT_PUBLIC_COMMON_GROUND` | `off` | Optional. Hides the CG badge when the Common Ground journal is not running |

### weave-api (VPS, `docker-compose.infra.yml`)

| Variable | Value | Notes |
|---|---|---|
| `DEMO_MODE` | `true` | Enables `POST /auth/demo/login` |
| `DEMO_LOGIN_LIMIT_PER_10_MIN` | `30` | Per-IP throttle on demo logins |
| `FRONTEND_URL` | `https://weave.mutref.tech` | Drives the cookie's `Secure` flag and OAuth redirects |
| `CORS_ALLOWED_ORIGINS` | `https://weave.mutref.tech` | Same origin as above |
| `SESSION_TTL_SECONDS` | `86400` | Demo sessions do not accumulate for a month |
| `HARD_CEILING_TOKENS_PER_DAY` | `1000000`–`2000000` | Caps spend, since the demo is public |
| `AUTH_STUB` | `false` | Never enable it: `/auth/test/login` accepts any email |

Each visitor gets their own account (`demo+<uuid>@weave.local`), because rate-limit
and quota counters are keyed by user id. A single shared demo user would make every
visitor compete for one 20-ingest/minute budget. The identity is never
bootstrap-admin eligible, and the handler refuses to issue a session for an admin.

## Deploying

1. **DNS**: in Cloudflare, add `CNAME weave → cname.vercel-dns.com` with the proxy
   **disabled** (grey cloud). Attach `weave.mutref.tech` to the `weave-frontend`
   project in Vercel.
2. **Vercel env**: add `NEXT_PUBLIC_DEMO_MODE=true`, then redeploy `master`.
3. **Backend**: set the weave-api variables above in the VPS `.env`, then rebuild
   only that service:

   ```bash
   docker compose -f docker-compose.infra.yml up -d --build weave-api
   docker compose -f docker-compose.infra.yml logs --tail=50 weave-api
   ```

   No new crates are involved, so Docker reuses the dependency layer and only
   recompiles the API crate.
4. **Verify**:

   ```bash
   node scripts/demo-smoke.mjs https://weave.mutref.tech
   API_URL=https://<api-host> node scripts/demo-smoke.mjs https://weave.mutref.tech
   ```

   The first run checks the public surface; adding `API_URL` also probes
   `/health/ready`, which verifies PostgreSQL and Redis. That route is not exposed
   through the frontend's rewrite rules.

Then open the URL in a private window: landing page → primary CTA → canvas with the
seeded Apollo graph, no sign-in.

## What the visitor sees

`/demo` mints the session, installs a seeded scene once per browser
(`frontend/src/lib/demo-graph.ts` → `seedDemoSession`), and redirects to `/app`.
The seed is a 25-concept, 45-edge Apollo program graph with authored positions,
provenance and `EXTRACTED`/`INFERRED`/`AMBIGUOUS` edges with evidence quotes, so the
inspector, insights panel and all four export formats work on first paint. Community
labels are not seeded: they are detected from the graph at runtime and named by the
API, so authored labels would key the wrong groups.

Seeding is idempotent and non-destructive: if the browser already holds a session
with content, the seed is skipped. Graph content lives in IndexedDB/localStorage, so
a shared demo login exposes no one else's work.

## Turning it off

1. Set `DEMO_MODE=false` on the API and restart it — `/auth/demo/login` returns 404.
2. Remove `NEXT_PUBLIC_DEMO_MODE` on Vercel and redeploy — the CTAs return to `/app`.

Existing demo sessions keep working until they expire (`SESSION_TTL_SECONDS`).
