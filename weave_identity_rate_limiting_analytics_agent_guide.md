# Weave --- Identity, Roles, Rate Limiting, Usage Metering & Analytics

## Agent Implementation Specification

**Repository:** `Sidharth-Singh10/weave`\
**Primary goal:** Add production-ready Google OAuth, persistent
users/roles, admin-controlled rate limits and quotas, per-user
overrides, usage metering, audit logs, and analytics without breaking
Weave's existing client-owned knowledge graph architecture.

------------------------------------------------------------------------

# 1. Mission

Implement an authentication and platform-control layer around the
existing Weave application.

The implementation must provide:

1.  Google OAuth login/logout.
2.  Persistent users and sessions.
3.  Admin-managed roles.
4.  Role-based permissions.
5.  Role-based rate limits and quotas.
6.  User-specific rate-limit/quota overrides.
7.  Endpoint-specific limits.
8.  Redis-backed distributed rate limiting.
9.  Usage/token metering for LLM operations.
10. Analytics events and aggregated analytics.
11. Admin dashboard for users, roles, policies, usage, and analytics.
12. Audit logging for administrative changes.
13. Security protections around authentication and login abuse.
14. Tests for all critical behavior.
15. Documentation and local development setup.

Do **not** move the knowledge graph from the browser to the server as
part of this work. The existing architecture intentionally keeps the
graph client-owned.

------------------------------------------------------------------------

# 2. Current Weave Architecture

The repository currently has:

-   `backend/` --- Rust/Axum/Tokio API.
-   `frontend/` --- Next.js 16 App Router application.
-   Zustand for the client-owned knowledge graph.
-   React Flow for graph visualization.
-   OpenAI-compatible LLM calls from the Rust backend.
-   Graph endpoints:
    -   `POST /api/graph/ingest`
    -   `POST /api/graph/organize`
    -   `POST /api/graph/label-community`
    -   `POST /api/graph/search`
    -   `GET /api/status`
    -   `GET /health`

The README explicitly describes the backend as stateless and the
knowledge graph as living in the browser. Preserve that design.

The new architecture should therefore become:

``` text
Google
   |
   v
OAuth / Authentication
   |
   v
Session
   |
   v
Next.js
   |
   v
Rust/Axum API
   |
   +--> Authentication
   |
   +--> User + Role resolution
   |
   +--> Effective policy resolution
   |
   +--> Redis rate limiting
   |
   +--> Usage metering
   |
   +--> Analytics events
   |
   +--> Existing graph/LLM handlers
   |
   +--> PostgreSQL
   |
   +--> Redis
```

Reference: current repository architecture and stack. citeturn0view0

------------------------------------------------------------------------

# 3. Non-Negotiable Architectural Decisions

## 3.1 Keep the graph client-owned

Do not introduce server-side persistence of graph nodes/edges merely to
implement authentication.

The server should know:

-   who made a request;
-   what endpoint was called;
-   request status;
-   latency;
-   usage/token counts;
-   high-level product events.

It should not automatically persist the contents of a user's knowledge
graph.

------------------------------------------------------------------------

## 3.2 Use PostgreSQL for durable state

PostgreSQL should be the source of truth for:

-   users;
-   roles;
-   permissions;
-   sessions;
-   rate-limit policies;
-   user overrides;
-   analytics event metadata;
-   audit logs.

Do not use Redis as the durable source of truth.

------------------------------------------------------------------------

## 3.3 Use Redis for hot-path counters

Redis should be used for:

-   request rate counters;
-   token counters;
-   concurrency counters;
-   short-lived login abuse counters;
-   optionally cached effective policies.

The application must remain correct if Redis is restarted, subject to
clearly documented fail-open/fail-closed behavior.

------------------------------------------------------------------------

## 3.4 Authentication and application rate limits are separate

There are two different systems:

### Authentication protection

Protect:

-   Google OAuth initiation;
-   OAuth callback;
-   session creation;
-   suspicious login activity.

### Application usage limits

Protect:

-   graph ingestion;
-   graph organization;
-   community labeling;
-   search;
-   other API operations.

Do not use the same policy for both.

------------------------------------------------------------------------

# 4. Recommended Technology Choices

Prefer mature libraries rather than implementing cryptography manually.

## Backend

Existing:

-   Rust
-   Axum
-   Tokio
-   serde
-   reqwest

Add appropriate crates for:

-   PostgreSQL access;
-   migrations;
-   session/cookie handling;
-   OAuth/OpenID verification;
-   Redis;
-   UUIDs;
-   timestamps;
-   passwordless session security;
-   structured logging.

The exact crate selection is an implementation decision. Before adding
dependencies, inspect the current `Cargo.toml`, Rust edition, existing
patterns, and lockfile.

Do not introduce a large authentication framework if it unnecessarily
conflicts with the current Axum architecture.

------------------------------------------------------------------------

# 5. Database Schema

Create migrations.

Do not rely on application startup code to silently create tables.

## 5.1 users

Suggested schema:

``` sql
users
-----
id UUID PRIMARY KEY
google_subject TEXT UNIQUE NOT NULL
email TEXT UNIQUE NOT NULL
name TEXT
avatar_url TEXT
role_id UUID NOT NULL
status TEXT NOT NULL
created_at TIMESTAMPTZ NOT NULL
updated_at TIMESTAMPTZ NOT NULL
last_login_at TIMESTAMPTZ
```

`google_subject` is the stable Google identity identifier.

Do not use email as the permanent external identity key.

Recommended statuses:

``` text
active
disabled
```

Optionally support:

``` text
suspended
```

if there is a real product requirement.

------------------------------------------------------------------------

# 6. Roles

Create:

``` sql
roles
-----
id UUID PRIMARY KEY
name TEXT UNIQUE NOT NULL
description TEXT
created_at TIMESTAMPTZ NOT NULL
updated_at TIMESTAMPTZ NOT NULL
```

Seed at least:

``` text
admin
member
```

Additional roles should be easy to create through the admin UI.

Example:

``` text
guest
member
researcher
admin
```

Do not hard-code every role in Rust.

Roles are database configuration.

------------------------------------------------------------------------

# 7. Permissions

Do not make the entire authorization system depend on role names.

Create permissions:

``` sql
permissions
-----------
id UUID PRIMARY KEY
key TEXT UNIQUE NOT NULL
description TEXT
```

Examples:

``` text
admin.users.read
admin.users.update
admin.roles.read
admin.roles.update
admin.policies.read
admin.policies.update
admin.analytics.read
admin.audit.read
graph.ingest
graph.organize
graph.label_community
graph.search
```

Create:

``` sql
role_permissions
----------------
role_id UUID
permission_id UUID
PRIMARY KEY (role_id, permission_id)
```

This allows an admin-created role to have selected capabilities without
modifying backend code.

------------------------------------------------------------------------

# 8. Sessions

Prefer server-side sessions represented by a secure HttpOnly cookie.

Suggested schema:

``` sql
sessions
--------
id UUID PRIMARY KEY
user_id UUID NOT NULL
session_token_hash TEXT UNIQUE NOT NULL
created_at TIMESTAMPTZ NOT NULL
expires_at TIMESTAMPTZ NOT NULL
last_seen_at TIMESTAMPTZ NOT NULL
revoked_at TIMESTAMPTZ
ip_hash TEXT
user_agent TEXT
```

Never store the raw session token in PostgreSQL.

Store a cryptographic hash.

The browser receives the raw opaque session token through an HttpOnly
cookie.

Recommended cookie properties:

``` text
HttpOnly
Secure in production
SameSite=Lax
Path=/
```

Do not put authentication tokens in `localStorage`.

------------------------------------------------------------------------

# 9. Google OAuth

Implement Authorization Code flow with Google's OAuth/OIDC endpoints.

Required environment variables should be documented in
`backend/.env.example`:

``` text
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GOOGLE_REDIRECT_URI=
SESSION_COOKIE_NAME=
SESSION_TTL_SECONDS=
FRONTEND_URL=
DATABASE_URL=
REDIS_URL=
```

The implementation should validate:

-   OAuth `state`;
-   redirect URI;
-   Google identity claims;
-   issuer;
-   audience/client ID;
-   stable Google subject.

Do not blindly trust an email returned by an arbitrary request.

------------------------------------------------------------------------

# 10. OAuth Flow

Expected flow:

``` text
GET /auth/google
        |
        v
Generate state
        |
        v
Store state securely
        |
        v
Redirect to Google
        |
        v
GET /auth/google/callback
        |
        +--> validate state
        |
        +--> exchange code
        |
        +--> validate identity
        |
        +--> find/create user
        |
        +--> check status
        |
        +--> create session
        |
        +--> set cookie
        |
        v
Redirect to frontend
```

Avoid leaking authorization codes, access tokens, or session tokens in
logs.

------------------------------------------------------------------------

# 11. First Admin Bootstrap

A new deployment needs a safe way to create the first admin.

Do not make the first Google user automatically admin unless explicitly
configured.

Recommended approach:

``` text
BOOTSTRAP_ADMIN_EMAILS=admin@example.com,owner@example.com
```

During first login:

-   if the authenticated email matches configured bootstrap admin
    emails;
-   and no admin exists;
-   assign the admin role.

After bootstrap, admin privileges must come from the database.

Document the behavior clearly.

Never expose an unauthenticated "make me admin" endpoint.

------------------------------------------------------------------------

# 12. Authentication Middleware

Create an authenticated request context.

Conceptually:

``` rust
struct UserContext {
    user_id: Uuid,
    role_id: Uuid,
    permissions: HashSet<String>,
}
```

Middleware should:

1.  Read the session cookie.
2.  Hash the token.
3.  Look up the session.
4.  Check expiration/revocation.
5.  Load the user.
6.  Check user status.
7.  Resolve role/permissions.
8.  Attach `UserContext` to the request.

Protected routes must fail with:

``` text
401 Unauthorized
```

when no valid session exists.

Disabled users must also be rejected.

------------------------------------------------------------------------

# 13. Authorization Middleware

Create reusable authorization checks.

Examples:

``` text
require_auth()
require_permission("graph.ingest")
require_permission("admin.users.read")
require_permission("admin.users.update")
```

Do not sprinkle:

``` rust
if user.role == "admin"
```

throughout handlers.

Use permissions.

------------------------------------------------------------------------

# 14. API Route Structure

Add authentication routes:

``` text
GET  /auth/google
GET  /auth/google/callback
POST /auth/logout
GET  /auth/me
```

Potentially:

``` text
POST /auth/logout-all
```

for revoking all sessions.

Protected graph routes:

``` text
POST /api/graph/ingest
POST /api/graph/organize
POST /api/graph/label-community
POST /api/graph/search
```

Admin routes:

``` text
GET   /api/admin/users
GET   /api/admin/users/:id
PATCH /api/admin/users/:id

GET   /api/admin/roles
POST  /api/admin/roles
PATCH /api/admin/roles/:id
DELETE /api/admin/roles/:id

GET   /api/admin/policies
PATCH /api/admin/policies/roles/:role_id
PATCH /api/admin/policies/users/:user_id

GET   /api/admin/analytics/overview
GET   /api/admin/analytics/users/:id
GET   /api/admin/audit
```

Exact routing style should follow the existing project conventions.

------------------------------------------------------------------------

# 15. Rate Limit Model

Rate limits must support:

-   global defaults;
-   role-specific limits;
-   user-specific overrides;
-   endpoint-specific policies;
-   multiple time windows;
-   token quotas;
-   concurrency limits.

Use an explicit model.

Example conceptual representation:

``` text
Policy
  scope: global | role | user
  role_id: optional
  user_id: optional

  endpoint: optional

  requests_per_minute
  requests_per_hour
  requests_per_day

  tokens_per_minute
  tokens_per_day
  tokens_per_month

  concurrent_requests
```

A more normalized schema is preferred if it makes policy management
cleaner:

``` sql
rate_limit_policies
-------------------
id
scope_type
role_id
user_id
endpoint
created_at
updated_at

rate_limit_rules
----------------
id
policy_id
metric
window
limit
```

Example:

``` text
metric=requests
window=minute
limit=60

metric=tokens
window=day
limit=2000000
```

This avoids schema changes when adding a new metric.

------------------------------------------------------------------------

# 16. Policy Resolution

The effective policy must follow:

``` text
Global default
      |
      v
Role policy
      |
      v
User override
```

More specific configuration wins.

For example:

``` text
Global:
requests/min = 30

Researcher:
requests/min = 60

Alice:
requests/min = 120
```

Alice gets:

``` text
120 requests/min
```

If Alice has no override:

``` text
60 requests/min
```

If the role has no value:

``` text
30 requests/min
```

------------------------------------------------------------------------

# 17. Global Hard Ceiling

Support a global safety ceiling.

For example:

``` text
Global hard maximum:
requests/min = 300
tokens/day = 20M
```

A user override of:

``` text
requests/min = 1000
```

must not exceed the global ceiling unless an explicit privileged bypass
mechanism exists.

Default behavior should be:

``` text
effective_limit = min(configured_limit, global_hard_limit)
```

This protects against accidental LLM cost explosions.

------------------------------------------------------------------------

# 18. Endpoint-Specific Policies

Different Weave operations have different costs.

At minimum distinguish:

``` text
graph.ingest
graph.organize
graph.label_community
graph.search
```

Example policy:

``` text
member:

ingest            20/min
organize          10/min
label-community   20/min
search            100/min
```

Do not treat all endpoints as equally expensive.

------------------------------------------------------------------------

# 19. Redis Rate Limiting

Use Redis for counters.

Conceptual keys:

``` text
rl:{user_id}:{endpoint}:minute
rl:{user_id}:{endpoint}:hour
rl:{user_id}:{endpoint}:day

tokens:{user_id}:day
tokens:{user_id}:month
```

Use atomic operations.

Avoid:

``` text
GET
calculate
SET
```

as separate non-atomic operations.

Prefer Redis Lua scripts or an equivalent atomic mechanism for
check-and-increment.

The algorithm may be:

-   fixed window for initial implementation;
-   sliding window/token bucket if needed later.

For the first production implementation, choose the simplest correct
algorithm and document it.

------------------------------------------------------------------------

# 20. Rate Limit Response

When blocked, return:

``` http
429 Too Many Requests
```

Include useful headers where possible:

``` text
Retry-After
X-RateLimit-Limit
X-RateLimit-Remaining
X-RateLimit-Reset
```

Response body:

``` json
{
  "error": "rate_limit_exceeded",
  "message": "Rate limit exceeded",
  "retry_after_seconds": 12
}
```

Do not expose internal Redis details.

------------------------------------------------------------------------

# 21. Login Abuse Protection

Protect OAuth endpoints separately.

Suggested controls:

``` text
OAuth initiation:
IP-based request limit

OAuth callback:
IP + state validation

Session creation:
user/account-based protection
```

Example starting values:

``` text
20 OAuth initiations / 10 minutes / IP
10 callbacks / 10 minutes / IP
```

These values are configurable.

Do not permanently ban an IP from a simple counter.

Prefer short-lived throttling.

------------------------------------------------------------------------

# 22. LLM Usage Metering

Rate limiting requests is not sufficient.

The expensive resource is LLM usage.

Record:

``` text
input_tokens
output_tokens
total_tokens
```

where the provider exposes token usage.

If token usage is unavailable:

-   do not invent fake precision;
-   optionally estimate and explicitly mark the value as estimated;
-   keep exact provider usage when available.

Each LLM operation should produce a usage record.

------------------------------------------------------------------------

# 23. Usage Records

Create:

``` sql
usage_events
------------
id UUID PRIMARY KEY
user_id UUID NOT NULL
request_id UUID
endpoint TEXT NOT NULL
provider TEXT
model TEXT

input_tokens BIGINT
output_tokens BIGINT
total_tokens BIGINT

latency_ms BIGINT
status_code INTEGER

created_at TIMESTAMPTZ NOT NULL
metadata JSONB
```

Do not put sensitive prompt/response contents in `metadata`.

------------------------------------------------------------------------

# 24. Analytics Events

Create a general event table:

``` sql
analytics_events
----------------
id UUID PRIMARY KEY
user_id UUID
event_type TEXT NOT NULL
timestamp TIMESTAMPTZ NOT NULL

request_id UUID
endpoint TEXT
metadata JSONB
```

Events should include:

``` text
login
logout
session_started
graph_ingest
graph_organize
graph_search
community_labeled
rate_limit_hit
quota_exceeded
```

Frontend events may include:

``` text
canvas_opened
first_note_created
graph_viewed
graph_node_edited
graph_node_deleted
```

Only collect product events that have a clear purpose.

------------------------------------------------------------------------

# 25. Privacy Rules

Do not store full knowledge graph contents as analytics.

Avoid storing:

-   raw notes;
-   complete graph node content;
-   complete LLM prompts;
-   complete LLM responses;
-   arbitrary sensitive frontend state.

Prefer:

``` text
nodes_created = 12
edges_created = 17
latency_ms = 820
tokens = 2100
```

Analytics should answer product and operational questions without
becoming a shadow copy of user data.

------------------------------------------------------------------------

# 26. Audit Logs

Create:

``` sql
audit_logs
----------
id UUID PRIMARY KEY
actor_user_id UUID NOT NULL
action TEXT NOT NULL
target_type TEXT
target_id UUID
old_value JSONB
new_value JSONB
created_at TIMESTAMPTZ NOT NULL
ip_hash TEXT
```

Log administrative actions such as:

``` text
user.role_changed
user.disabled
user.enabled
role.created
role.updated
role.deleted
role.permissions_changed
policy.role_updated
policy.user_override_updated
policy.user_override_removed
```

Do not log secrets.

Do not store OAuth tokens.

------------------------------------------------------------------------

# 27. Admin UI

Add an authenticated admin area in the Next.js frontend.

Suggested route:

``` text
/admin
```

Sections:

``` text
Overview
Users
Roles
Policies
Usage
Analytics
Audit Log
```

------------------------------------------------------------------------

# 28. Admin Overview

Show:

``` text
Total users
Active users
New users
Requests today
LLM tokens today
Rate-limit violations
```

Charts:

``` text
Daily active users
Requests/day
LLM tokens/day
New users/day
```

Use server-side admin API calls.

Do not query PostgreSQL directly from the browser.

------------------------------------------------------------------------

# 29. User Management

Admin user table:

``` text
Email
Name
Role
Status
Last login
Requests today
Tokens today
Created
```

Actions:

``` text
Change role
Disable
Enable
View usage
View activity
Configure limits
```

Prevent an admin from accidentally removing the final admin unless
another admin remains.

------------------------------------------------------------------------

# 30. Role Management

Role page:

``` text
Role name
Description

Permissions
[ ] graph.ingest
[ ] graph.organize
[ ] graph.search
[ ] admin.users.read
...

Rate limits
Requests/min
Requests/hour
Requests/day
Tokens/min
Tokens/day
Tokens/month
Concurrent requests
```

Saving changes should:

1.  Validate values.
2.  Update PostgreSQL.
3.  Invalidate any cached policy.
4.  Write an audit log.

------------------------------------------------------------------------

# 31. User-Specific Overrides

User page should show:

``` text
Role: Researcher

Inherited:
requests/min = 60
tokens/day = 2M

Overrides:
requests/min = 120
tokens/day = 10M
```

Actions:

``` text
Set override
Remove override
Reset to role
```

The UI should clearly distinguish:

``` text
Inherited from role
```

from:

``` text
Explicit user override
```

------------------------------------------------------------------------

# 32. Analytics Dashboard

Admin analytics should include:

## User metrics

``` text
Total users
DAU
WAU
MAU
New users
Returning users
Retention
```

## API metrics

``` text
Requests by endpoint
Requests by status
Average latency
p95 latency
Rate-limit hits
```

## LLM metrics

``` text
Input tokens
Output tokens
Total tokens
Usage by model
Usage by endpoint
Usage by user
```

## Product metrics

``` text
Users who opened canvas
Users who created first note
Users who generated first graph
Users returning after 1 day
Users returning after 7 days
```

------------------------------------------------------------------------

# 33. Analytics Aggregation

Do not calculate expensive analytics by scanning millions of raw events
on every admin page load.

Start with PostgreSQL indexes and grouped queries.

If scale requires it, add daily aggregates:

``` sql
analytics_daily
---------------
date
metric
dimension
dimension_value
value
```

Examples:

``` text
2026-08-12
active_users
all
NULL
128

2026-08-12
llm_tokens
model
deepseek-v4-flash
12000000
```

Only introduce aggregation complexity when needed.

------------------------------------------------------------------------

# 34. Required Indexes

At minimum:

``` sql
users(email)
users(google_subject)
users(role_id)
users(status)

sessions(session_token_hash)
sessions(user_id)
sessions(expires_at)

analytics_events(user_id, timestamp)
analytics_events(event_type, timestamp)
analytics_events(timestamp)

usage_events(user_id, created_at)
usage_events(endpoint, created_at)

audit_logs(actor_user_id, created_at)
audit_logs(target_id, created_at)
```

Use composite indexes based on actual query patterns.

------------------------------------------------------------------------

# 35. Frontend Authentication

Add an auth client.

Conceptually:

``` text
src/lib/auth.ts
src/lib/api.ts
src/components/auth/
```

The frontend should be able to call:

``` text
GET /auth/me
```

and receive:

``` json
{
  "authenticated": true,
  "user": {
    "id": "...",
    "email": "...",
    "name": "...",
    "avatar_url": "...",
    "role": "member"
  }
}
```

For unauthenticated users:

``` json
{
  "authenticated": false
}
```

Do not put sensitive authorization logic solely in React state.

The server remains authoritative.

------------------------------------------------------------------------

# 36. Login UI

Create a simple login page.

Minimum:

``` text
Weave

[ Continue with Google ]
```

After successful login:

``` text
/app
```

If a user is disabled:

``` text
Your account has been disabled.
```

Do not reveal internal administrative details.

------------------------------------------------------------------------

# 37. User Menu

Authenticated canvas UI should expose:

``` text
User avatar
Name
Email

Account
Logout
```

Admins additionally see:

``` text
Admin Dashboard
```

The backend must still enforce admin permissions even if the frontend
hides the button.

------------------------------------------------------------------------

# 38. Handling Expired Sessions

If the API returns:

``` text
401
```

the frontend should:

1.  Clear client authentication state.
2.  Redirect to login.
3.  Preserve the intended destination where appropriate.

Avoid infinite redirect loops.

------------------------------------------------------------------------

# 39. Graph API Integration

All existing graph endpoints that consume LLM resources should become
authenticated.

Example:

``` text
POST /api/graph/ingest
```

pipeline:

``` text
Request
  ↓
Auth
  ↓
Permission
  ↓
Resolve policy
  ↓
Rate limit request
  ↓
LLM call
  ↓
Record usage
  ↓
Record analytics
  ↓
Response
```

Search may have a different policy because it may be cheaper.

------------------------------------------------------------------------

# 40. Request ID

Introduce a request ID.

Every request should have:

``` text
request_id: UUID
```

Accept an incoming request ID only if safely validated; otherwise
generate one.

Return:

``` text
X-Request-ID
```

Use the request ID in:

-   logs;
-   usage records;
-   analytics;
-   error responses.

This will make debugging dramatically easier.

------------------------------------------------------------------------

# 41. Error Model

Standardize API errors.

Example:

``` json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded",
    "request_id": "..."
  }
}
```

Useful codes:

``` text
unauthorized
forbidden
rate_limit_exceeded
quota_exceeded
invalid_request
not_found
conflict
internal_error
```

Do not expose stack traces in production.

------------------------------------------------------------------------

# 42. Database Transactions

Use transactions for administrative changes that affect multiple tables.

Example role update:

``` text
BEGIN
  update role
  update permissions
  insert audit log
COMMIT
```

If any step fails, rollback.

------------------------------------------------------------------------

# 43. Policy Cache Invalidation

If effective policies are cached:

``` text
role updated
   ↓
invalidate role policy cache

user override updated
   ↓
invalidate user policy cache
```

Never allow stale policies to persist indefinitely.

Use short TTLs as a safety net.

------------------------------------------------------------------------

# 44. Redis Failure Behavior

Choose and document behavior.

Recommended:

### Authentication

Fail closed.

### Rate limiting

For expensive LLM endpoints, fail closed or use a conservative fallback.

Do not silently allow unlimited LLM requests because Redis is
unavailable.

For cheap health/status endpoints, Redis should not be required.

------------------------------------------------------------------------

# 45. Concurrency Limits

Support:

``` text
concurrent_requests
```

for expensive operations.

Example:

``` text
member:
2 concurrent ingest operations

researcher:
5

admin:
10
```

Use Redis counters with expiration/finally cleanup.

Make sure counters are released even when the request fails.

In Rust, use RAII-style guards where appropriate.

------------------------------------------------------------------------

# 46. Quota Semantics

Clearly define:

### Requests/minute

Number of completed or accepted requests in a window.

### Tokens/day

Total provider-reported tokens consumed.

### Concurrent requests

Requests currently executing.

Document these semantics in code and admin UI.

Do not let different endpoints interpret the same quota differently.

------------------------------------------------------------------------

# 47. Security Requirements

The agent must verify:

-   OAuth state validation.
-   Secure cookies.
-   Session expiration.
-   Session revocation.
-   Disabled users rejected.
-   Permission checks server-side.
-   Admin routes protected.
-   CSRF protection where relevant.
-   CORS restricted to the real frontend origin.
-   No wildcard production CORS with credentials.
-   No OAuth secrets committed to Git.
-   No session tokens logged.
-   No provider API keys logged.
-   No raw prompts/responses in analytics.
-   SQL queries parameterized.
-   Redis keys safely constructed.
-   Input limits on large requests.
-   Admin actions audited.

------------------------------------------------------------------------

# 48. CORS and Cookie Deployment

Because frontend and Rust backend are separate applications, explicitly
design the deployment topology.

Preferred production setup:

``` text
https://weave.example.com
        |
        +--> frontend
        |
        +--> /api and /auth
              |
              v
            Rust API
```

A same-site reverse proxy is strongly preferred.

This makes secure cookie handling much easier than unrelated domains.

For local development, document the separate frontend/backend origins
and the cookie/CORS configuration required.

------------------------------------------------------------------------

# 49. Environment Configuration

Update:

``` text
backend/.env.example
```

with all new settings.

Example:

``` text
DATABASE_URL=
REDIS_URL=

GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GOOGLE_REDIRECT_URI=

FRONTEND_URL=
SESSION_COOKIE_NAME=weave_session
SESSION_TTL_SECONDS=2592000

BOOTSTRAP_ADMIN_EMAILS=

RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE=30
RATE_LIMIT_DEFAULT_TOKENS_PER_DAY=500000
```

Do not hard-code production values.

------------------------------------------------------------------------

# 50. Database Migrations

Add migrations in a dedicated directory following the chosen Rust
database library's convention.

Migration order should be approximately:

``` text
001_create_roles
002_create_permissions
003_create_role_permissions
004_create_users
005_create_sessions
006_create_rate_limit_policies
007_create_rate_limit_rules
008_create_usage_events
009_create_analytics_events
010_create_audit_logs
```

Add seed/bootstrap logic separately from schema creation.

------------------------------------------------------------------------

# 51. Testing Strategy

Tests are mandatory.

## Unit tests

Test:

``` text
policy resolution
permission checks
quota calculations
rate limit calculations
session expiration
role inheritance
user override precedence
global hard ceiling
```

Example:

``` text
global = 30
role = 60
user = 120
hard ceiling = 100

effective = 100
```

------------------------------------------------------------------------

# 52. Authentication Tests

Test:

``` text
valid OAuth identity
invalid OAuth state
invalid issuer
invalid audience
unknown user creation
existing user login
disabled user
expired session
revoked session
logout
logout-all
```

Do not make tests depend on real Google.

Mock the OAuth identity provider.

------------------------------------------------------------------------

# 53. Authorization Tests

Test:

``` text
member cannot access admin
admin can access admin
role permission grants access
permission removed denies access
disabled user denied
unauthenticated user denied
```

------------------------------------------------------------------------

# 54. Rate Limiting Tests

Test:

``` text
under limit succeeds
limit boundary succeeds/fails correctly
over limit returns 429
minute window resets
daily quota blocks
user override works
role policy inheritance works
endpoint-specific policy works
global ceiling works
```

Test concurrent requests.

Test Redis failure behavior.

------------------------------------------------------------------------

# 55. Analytics Tests

Verify that:

``` text
login creates login event
graph ingest creates usage event
LLM usage is recorded
rate-limit violation is recorded
admin changes create audit logs
```

Analytics failure should not break the core graph operation unless the
event is legally/business-critical.

Prefer asynchronous or failure-isolated analytics where practical.

------------------------------------------------------------------------

# 56. Frontend Tests

Test:

``` text
login button
authenticated state
unauthenticated redirect
logout
admin visibility
admin route protection
rate-limit error UI
quota exceeded UI
expired session handling
```

------------------------------------------------------------------------

# 57. End-to-End Test

Create at least one complete flow:

``` text
Google login (mocked)
       ↓
user created
       ↓
member role assigned
       ↓
open app
       ↓
ingest graph note
       ↓
usage recorded
       ↓
analytics recorded
       ↓
admin changes role
       ↓
admin changes user quota
       ↓
user receives new effective policy
       ↓
quota eventually blocks request
```

------------------------------------------------------------------------

# 58. Admin Safety

The system must prevent dangerous states.

At minimum:

1.  Do not allow deleting the last admin role.
2.  Do not allow removing the last admin's admin permissions without
    another admin existing.
3.  Do not allow an admin to disable the final active admin.
4.  Do not allow negative rate limits.
5.  Do not allow nonsensical quota values.
6.  Do not allow user override to bypass hard global limits.
7.  Audit every privileged change.

------------------------------------------------------------------------

# 59. Observability

Add structured logs.

Useful fields:

``` text
request_id
user_id
endpoint
status
latency_ms
role
rate_limit_result
tokens
```

Never log:

``` text
session token
OAuth code
Google access token
provider API key
raw user notes
raw LLM prompt
raw LLM response
```

------------------------------------------------------------------------

# 60. Metrics

If an existing metrics system is not present, do not introduce a huge
observability stack just for this feature.

Start with database analytics plus structured logs.

Potential future metrics:

``` text
http_requests_total
http_request_duration
llm_tokens_total
rate_limit_hits_total
auth_success_total
auth_failure_total
active_sessions
```

------------------------------------------------------------------------

# 61. API Documentation

Document every new API route.

For each route include:

``` text
Authentication required?
Permission required?
Request body
Response
Errors
Rate limit
```

Example:

``` text
POST /api/graph/ingest

Auth: required
Permission: graph.ingest
Rate limit: policy-dependent

429:
rate_limit_exceeded

401:
unauthorized
```

------------------------------------------------------------------------

# 62. Admin API Design Principle

Never expose raw database structures unnecessarily.

Instead of:

``` text
GET /api/admin/rate_limit_policies
```

return an API representation that makes inheritance clear:

``` json
{
  "role": "researcher",
  "limits": {
    "requests_per_minute": 60,
    "tokens_per_day": 2000000
  }
}
```

For users:

``` json
{
  "user": "...",
  "role": "researcher",
  "inherited": {
    "requests_per_minute": 60
  },
  "overrides": {
    "requests_per_minute": 120
  },
  "effective": {
    "requests_per_minute": 120
  }
}
```

This makes the admin UI much easier to implement correctly.

------------------------------------------------------------------------

# 63. Migration Compatibility

Existing Weave users currently have no accounts.

Therefore:

-   the new authentication system should not assume an existing user
    table;
-   first login creates an account;
-   existing local graph data should remain available in the browser;
-   login should not wipe Zustand state;
-   adding authentication must not silently reset the graph.

This is critical.

------------------------------------------------------------------------

# 64. Existing Graph Persistence

Before modifying authentication-related frontend state, inspect the
current Zustand persistence implementation.

The authentication store should be separate from the graph store.

Prefer:

``` text
authStore
graphStore
```

rather than combining them.

Authentication lifecycle must not cause:

``` text
graphStore.reset()
```

unless explicitly requested by the user.

------------------------------------------------------------------------

# 65. Frontend API Client

Centralize API behavior.

The API client should handle:

``` text
credentials: include
401 handling
429 handling
request IDs
standardized errors
```

Do not duplicate cookie/auth handling in every component.

------------------------------------------------------------------------

# 66. Rate Limit UX

When the user hits a limit, the UI should explain:

``` text
You've reached your current usage limit.

Try again in 42 seconds.
```

For daily quota:

``` text
You've reached today's AI usage limit.
```

Do not expose internal policy names.

If the user is an admin, show relevant usage information but do not leak
Redis implementation details.

------------------------------------------------------------------------

# 67. Admin Analytics UX

Admin dashboard should support date ranges:

``` text
Today
7 days
30 days
90 days
Custom
```

Do not fetch all raw analytics events into the browser.

The backend should return aggregated data.

------------------------------------------------------------------------

# 68. Pagination

Admin lists must be paginated:

``` text
users
audit logs
usage events
```

Do not return thousands of records by default.

Support:

``` text
limit
cursor
```

or page-based pagination.

Cursor pagination is preferred for large audit/event tables.

------------------------------------------------------------------------

# 69. Search and Filtering

User admin page should support:

``` text
search email/name
filter role
filter status
sort last login
sort usage
```

Audit logs:

``` text
filter actor
filter action
filter target
date range
```

Analytics:

``` text
date range
endpoint
role
user
model
```

------------------------------------------------------------------------

# 70. Avoid Overengineering

Do not implement:

-   billing;
-   Stripe;
-   organization/team accounts;
-   email/password authentication;
-   multiple OAuth providers;
-   graph server persistence;
-   complex event streaming;
-   Kafka;
-   ClickHouse;

unless the existing repository or product requirement makes them
necessary.

The current goal is identity + policy + metering + analytics.

------------------------------------------------------------------------

# 71. Suggested Implementation Order

Implement in these phases.

## Phase 0 --- Repository audit

Before writing code:

1.  Inspect the complete repository.
2.  Inspect backend `Cargo.toml`.
3.  Inspect frontend `package.json`.
4.  Inspect current API routes.
5.  Inspect Zustand persistence.
6.  Inspect existing environment/config patterns.
7.  Inspect Docker setup.
8.  Inspect existing tests.
9.  Inspect existing documentation/iteration guides.
10. Produce a short implementation plan based on actual files.

Do not assume the repository matches this document perfectly.

------------------------------------------------------------------------

## Phase 1 --- Infrastructure

Implement:

-   PostgreSQL connection.
-   Redis connection.
-   migrations.
-   configuration.
-   health checks.

Add:

``` text
GET /health
```

checks appropriate dependencies without making health unnecessarily
unusable during local development.

------------------------------------------------------------------------

## Phase 2 --- Identity

Implement:

-   Google OAuth;
-   users;
-   sessions;
-   auth middleware;
-   `/auth/me`;
-   logout;
-   bootstrap admin.

Do not modify graph behavior beyond requiring authentication where
intended.

------------------------------------------------------------------------

## Phase 3 --- Authorization

Implement:

-   roles;
-   permissions;
-   role-permission mapping;
-   permission middleware;
-   admin routes.

------------------------------------------------------------------------

## Phase 4 --- Rate limiting

Implement:

-   policy schema;
-   global defaults;
-   role policies;
-   user overrides;
-   Redis counters;
-   endpoint-specific limits;
-   token quotas;
-   concurrency limits;
-   429 responses.

------------------------------------------------------------------------

## Phase 5 --- Usage metering

Implement:

-   request IDs;
-   usage events;
-   token accounting;
-   request latency;
-   endpoint usage.

------------------------------------------------------------------------

## Phase 6 --- Analytics

Implement:

-   analytics events;
-   aggregation queries;
-   overview endpoint;
-   user analytics;
-   product funnel metrics.

------------------------------------------------------------------------

## Phase 7 --- Admin UI

Implement:

-   overview;
-   users;
-   roles;
-   policies;
-   user overrides;
-   analytics;
-   audit log.

------------------------------------------------------------------------

## Phase 8 --- Hardening

Perform:

-   security review;
-   race-condition review;
-   Redis failure testing;
-   session testing;
-   authorization review;
-   rate-limit bypass testing;
-   CORS/cookie review;
-   input validation review.

------------------------------------------------------------------------

# 72. Definition of Done

The implementation is complete only when all of the following are true.

## Authentication

-   [ ] Google login works.
-   [ ] OAuth state is validated.
-   [ ] Google identity is validated.
-   [ ] Sessions use secure HttpOnly cookies.
-   [ ] Logout works.
-   [ ] Session expiry works.
-   [ ] Session revocation works.
-   [ ] Disabled users cannot authenticate.

## Authorization

-   [ ] Roles are persisted.
-   [ ] Permissions are persisted.
-   [ ] Admin routes require permissions.
-   [ ] Role changes take effect without redeployment.
-   [ ] Final-admin protections exist.

## Rate limiting

-   [ ] Global defaults exist.
-   [ ] Role policies exist.
-   [ ] User overrides exist.
-   [ ] Endpoint-specific limits exist.
-   [ ] Redis-backed counters work.
-   [ ] 429 responses work.
-   [ ] Retry information is returned.
-   [ ] Token quotas work.
-   [ ] Concurrency limits work.
-   [ ] Global hard ceilings work.

## Analytics

-   [ ] Login events recorded.
-   [ ] Graph usage recorded.
-   [ ] Token usage recorded.
-   [ ] Rate-limit events recorded.
-   [ ] Admin changes audited.
-   [ ] Admin analytics dashboard works.
-   [ ] User-level usage works.

## Security

-   [ ] Secrets not committed.
-   [ ] Tokens not logged.
-   [ ] OAuth credentials not logged.
-   [ ] Raw user graph contents not stored as analytics.
-   [ ] CORS is restricted.
-   [ ] SQL is parameterized.
-   [ ] Admin authorization is server-side.

## Compatibility

-   [ ] Existing graph functionality still works.
-   [ ] Existing Zustand graph state is not wiped by login.
-   [ ] Existing mock LLM mode still works where applicable.
-   [ ] Existing frontend build succeeds.
-   [ ] Existing backend tests pass.

------------------------------------------------------------------------

# 73. Agent Working Rules

The implementing agent MUST follow these rules.

## Rule 1 --- Inspect before editing

Never invent file paths.

Inspect the actual repository first.

------------------------------------------------------------------------

## Rule 2 --- Preserve existing behavior

The authentication feature is an addition, not a rewrite of Weave.

Avoid unnecessary refactors.

------------------------------------------------------------------------

## Rule 3 --- Small commits/steps

Implement in logical chunks:

``` text
auth
database
roles
rate limits
usage
analytics
admin UI
hardening
```

Keep changes reviewable.

------------------------------------------------------------------------

## Rule 4 --- Run tests continuously

After backend changes:

``` bash
cargo test
```

After frontend changes:

``` bash
npm run lint
npm run build
```

Run the project's actual commands if they differ.

------------------------------------------------------------------------

## Rule 5 --- Never bypass authorization for convenience

Do not rely on:

``` text
frontend hiding admin button
```

as security.

Every privileged backend operation must verify permissions.

------------------------------------------------------------------------

## Rule 6 --- Do not silently change product semantics

If a design decision affects:

-   graph ownership;
-   graph persistence;
-   privacy;
-   quota semantics;
-   admin capabilities;

stop and document the decision rather than silently changing behavior.

------------------------------------------------------------------------

# 74. Final Expected Architecture

The completed system should conceptually look like:

``` text
                         Google
                           |
                           v
                    +-------------+
                    | OAuth/OIDC  |
                    +------+------+
                           |
                           v
                    +-------------+
                    |   Session   |
                    +------+------+
                           |
                           v
                    +-------------+
                    | User + Role |
                    +------+------+
                           |
                           v
                    +-------------+
                    | Permissions |
                    +------+------+
                           |
                           v
                    +-------------+
                    |   Policy    |
                    | Resolution  |
                    +------+------+
                           |
                           v
                    +-------------+
                    |    Redis    |
                    | Rate/Quota  |
                    +------+------+
                           |
                           v
                    +-------------+
                    | Axum Graph  |
                    |    API      |
                    +------+------+
                           |
                 +---------+---------+
                 |                   |
                 v                   v
            PostgreSQL            LLM
                 |
       +---------+----------+
       |         |          |
       v         v          v
     Users    Analytics    Audit
     Roles     Usage       Logs
```

The browser remains:

``` text
                     Browser
                        |
            +-----------+-----------+
            |                       |
            v                       v
       Auth/session             Graph store
                                Zustand
                                   |
                                React Flow
```

The graph remains client-owned while identity, authorization, usage
controls, and platform analytics live server-side.

------------------------------------------------------------------------

# 75. Recommended First Task for the Agent

Before implementing anything, produce a repository audit containing:

``` text
1. Current backend structure
2. Current frontend structure
3. Existing API routes
4. Existing state persistence
5. Current environment configuration
6. Existing database/Redis dependencies, if any
7. Existing authentication, if any
8. Existing tests
9. Docker/deployment topology
10. Exact files that need modification
11. Exact files that need creation
12. Proposed dependency additions
13. Migration plan
14. Risks/conflicts with current architecture
```

Then implement Phase 1.

Do not begin by changing the frontend login screen.

The correct order is:

``` text
Repository audit
      ↓
Infrastructure
      ↓
Database
      ↓
Authentication
      ↓
Authorization
      ↓
Rate limiting
      ↓
Usage metering
      ↓
Analytics
      ↓
Admin UI
      ↓
Testing + hardening
```

This order keeps the system coherent and prevents the frontend from
being built around authentication semantics that the backend has not yet
defined.
