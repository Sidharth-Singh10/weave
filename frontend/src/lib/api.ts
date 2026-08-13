import type { ApiEdge, GraphDelta, IngestRequest } from "./graph-types";

const API_BASE = process.env.NEXT_PUBLIC_WEAVE_API_URL ?? "";

/** Standardized backend error (see the backend error model). */
export class ApiError extends Error {
  code: string;
  status: number;
  requestId?: string;
  retryAfter?: number;

  constructor(
    code: string,
    message: string,
    status: number,
    requestId?: string,
    retryAfter?: number
  ) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
    this.requestId = requestId;
    this.retryAfter = retryAfter;
  }

  get isRateLimited() {
    return this.code === "rate_limit_exceeded" || this.code === "quota_exceeded";
  }
}

/** Hooked by the auth store: called whenever any request returns 401. */
let unauthorizedHandler: (() => void) | null = null;
export function setUnauthorizedHandler(fn: (() => void) | null) {
  unauthorizedHandler = fn;
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init.headers ?? {}) },
    credentials: "include",
  });

  const requestId = res.headers.get("x-request-id") ?? undefined;

  if (res.status === 401) {
    unauthorizedHandler?.();
  }

  if (!res.ok) {
    let code = "error";
    let message = res.statusText;
    try {
      const body = await res.json();
      if (body?.error?.code) code = body.error.code;
      if (body?.error?.message) message = body.error.message;
    } catch {
      /* non-JSON error body */
    }
    const retryRaw = res.headers.get("retry-after");
    const retryAfter = retryRaw ? Number(retryRaw) : undefined;
    throw new ApiError(code, message, res.status, requestId, retryAfter);
  }

  const ct = res.headers.get("content-type") ?? "";
  if (ct.includes("application/json")) {
    return (await res.json()) as T;
  }
  return (await res.text()) as unknown as T;
}

// ---------------------------------------------------------------------------
// Graph API
// ---------------------------------------------------------------------------

export async function ingestNote(req: IngestRequest): Promise<GraphDelta> {
  return request<GraphDelta>("/api/graph/ingest", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export interface LabelCommunityResponse {
  label: string;
}

export interface OrganizeGroup {
  label: string;
  member_labels: string[];
}

export interface OrganizeResult {
  groups: OrganizeGroup[];
  missing_edges: ApiEdge[];
  disconnected: string[];
  duplicates: { label_a: string; label_b: string }[];
}

/** Advisory AI analysis of the graph. Always optional — callers degrade
 * gracefully to an empty result on failure. */
export async function organizeGraph(
  nodes: { id?: string; label: string; kind: string }[],
  edges: {
    id?: string;
    source_label: string;
    target_label: string;
    relation: string;
  }[]
): Promise<OrganizeResult> {
  return request<OrganizeResult>("/api/graph/organize", {
    method: "POST",
    body: JSON.stringify({ nodes, edges }),
  });
}

/** Ask the LLM to name a community from its member node labels. */
export async function labelCommunity(nodes: string[]): Promise<string> {
  const data = await request<LabelCommunityResponse>("/api/graph/label-community", {
    method: "POST",
    body: JSON.stringify({ nodes }),
  });
  return data.label;
}

// ---------------------------------------------------------------------------
// Auth API
// ---------------------------------------------------------------------------

export interface AuthUser {
  id: string;
  email: string;
  name: string | null;
  avatar_url: string | null;
  role: string;
}

export interface AuthMe {
  authenticated: boolean;
  user?: AuthUser;
}

export async function getMe(): Promise<AuthMe> {
  return request<AuthMe>("/auth/me");
}

export async function postLogout(): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/auth/logout", { method: "POST" });
}

// ---------------------------------------------------------------------------
// Admin API
// ---------------------------------------------------------------------------

export interface AdminUserItem {
  id: string;
  email: string;
  name: string | null;
  avatar_url: string | null;
  role_id: string;
  role: string;
  status: string;
  created_at: string;
  last_login_at: string | null;
}

export async function adminListUsers(params: {
  page?: number;
  page_size?: number;
  search?: string;
  role?: string;
  status?: string;
}): Promise<{ items: AdminUserItem[]; total: number; page: number; page_size: number }> {
  const q = new URLSearchParams();
  if (params.page) q.set("page", String(params.page));
  if (params.page_size) q.set("page_size", String(params.page_size));
  if (params.search) q.set("search", params.search);
  if (params.role) q.set("role", params.role);
  if (params.status) q.set("status", params.status);
  const qs = q.toString();
  return request(`/api/admin/users${qs ? `?${qs}` : ""}`);
}

export async function adminUpdateUser(
  id: string,
  patch: { role_id?: string; status?: string; name?: string }
): Promise<{ ok: boolean }> {
  return request(`/api/admin/users/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
}

export interface AdminRole {
  id: string;
  name: string;
  description: string | null;
  permissions: string[];
}

export async function adminListRoles(): Promise<{ roles: AdminRole[] }> {
  return request("/api/admin/roles");
}

export async function adminCreateRole(body: {
  name: string;
  description?: string;
  permission_keys: string[];
}): Promise<{ id: string }> {
  return request("/api/admin/roles", { method: "POST", body: JSON.stringify(body) });
}

export async function adminUpdateRole(
  id: string,
  body: { name?: string; description?: string; permission_keys?: string[] }
): Promise<{ ok: boolean }> {
  return request(`/api/admin/roles/${id}`, { method: "PATCH", body: JSON.stringify(body) });
}

export async function adminDeleteRole(id: string): Promise<{ ok: boolean }> {
  return request(`/api/admin/roles/${id}`, { method: "DELETE" });
}

export interface Limits {
  requests_per_minute?: number;
  requests_per_hour?: number;
  requests_per_day?: number;
  tokens_per_minute?: number;
  tokens_per_day?: number;
  tokens_per_month?: number;
  concurrent_requests?: number;
}

export interface AdminPolicies {
  global: Limits;
  roles: { role_id: string; role: string; limits: Limits; endpoints: Record<string, Limits> }[];
  users: { user_id: string; email: string; role: string; overrides: Limits }[];
}

export async function adminGetPolicies(): Promise<AdminPolicies> {
  return request("/api/admin/policies");
}

export async function adminPatchRolePolicy(
  roleId: string,
  limits: Limits
): Promise<{ ok: boolean }> {
  return request(`/api/admin/policies/roles/${roleId}`, {
    method: "POST",
    body: JSON.stringify({ limits }),
  });
}

export async function adminPatchUserPolicy(
  userId: string,
  limits: Limits
): Promise<{ ok: boolean }> {
  return request(`/api/admin/policies/users/${userId}`, {
    method: "POST",
    body: JSON.stringify({ limits }),
  });
}

export interface AdminOverview {
  period_days: number;
  totals: {
    total_users: number;
    active_users: number;
    new_users: number;
    requests_today: number;
    llm_tokens_today: number;
    rate_limit_hits: number;
  };
  charts: {
    active_users: { date: string; value: number }[];
    requests: { date: string; value: number }[];
    llm_tokens: { date: string; value: number }[];
    new_users: { date: string; value: number }[];
  };
  llm: {
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    by_model: { model: string; tokens: number }[];
    by_endpoint: { endpoint: string; requests: number; tokens: number }[];
  };
  api: { avg_latency_ms: number | null; p95_latency_ms: number | null };
  top_users: { email: string; requests: number; tokens: number }[];
}

export async function adminOverview(days = 30): Promise<AdminOverview> {
  return request(`/api/admin/analytics/overview?days=${days}`);
}

export interface AdminAuditItem {
  id: string;
  action: string;
  actor_email: string | null;
  target_type: string | null;
  target_id: string | null;
  old_value: unknown;
  new_value: unknown;
  created_at: string;
}

export async function adminAudit(params: {
  limit?: number;
  cursor?: string;
  action?: string;
}): Promise<{ items: AdminAuditItem[]; next_cursor: string | null }> {
  const q = new URLSearchParams();
  if (params.limit) q.set("limit", String(params.limit));
  if (params.cursor) q.set("cursor", params.cursor);
  if (params.action) q.set("action", params.action);
  const qs = q.toString();
  return request(`/api/admin/audit${qs ? `?${qs}` : ""}`);
}
