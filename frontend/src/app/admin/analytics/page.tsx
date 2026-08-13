"use client";

import { useState } from "react";
import { adminOverview, adminListUsers, type AdminUserItem } from "@/lib/api";
import { useAdmin, AdminState, Card, SimpleBar } from "@/components/admin/useAdmin";

const RANGES = [7, 30, 90];

export default function AnalyticsPage() {
  const [days, setDays] = useState(30);
  const { data, error, loading } = useAdmin(() => adminOverview(days), [days]);

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-lg font-semibold tracking-tight">Analytics</h1>
        <div className="flex gap-1 rounded-lg border border-line bg-surface p-0.5">
          {RANGES.map((r) => (
            <button
              key={r}
              onClick={() => setDays(r)}
              className={[
                "rounded-md px-2.5 py-1 text-xs font-medium",
                days === r ? "bg-accent text-accent-ink" : "text-muted hover:text-foreground",
              ].join(" ")}
            >
              {r}d
            </button>
          ))}
        </div>
      </div>

      <AdminState loading={loading} error={error}>
        {data && (
          <div className="space-y-6">
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              <Card title="Total tokens" value={data.llm.total_tokens.toLocaleString()} sub={`last ${days}d`} />
              <Card title="Input tokens" value={data.llm.input_tokens.toLocaleString()} />
              <Card title="Output tokens" value={data.llm.output_tokens.toLocaleString()} />
              <Card title="Avg latency" value={data.api.avg_latency_ms ? `${Math.round(data.api.avg_latency_ms)} ms` : "–"} />
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="rounded-xl border border-line bg-surface p-4">
                <h2 className="mb-3 text-sm font-medium">Tokens / day</h2>
                <SimpleBar data={data.charts.llm_tokens} />
              </div>
              <div className="rounded-xl border border-line bg-surface p-4">
                <h2 className="mb-3 text-sm font-medium">Requests by endpoint</h2>
                <div className="space-y-1.5">
                  {data.llm.by_endpoint.map((e) => (
                    <div key={e.endpoint} className="flex items-center justify-between text-sm">
                      <span className="text-muted">{e.endpoint}</span>
                      <span>
                        {e.requests} req · {e.tokens.toLocaleString()} tok
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-line bg-surface p-4">
              <h2 className="mb-3 text-sm font-medium">Usage by model</h2>
              <div className="space-y-1.5">
                {data.llm.by_model.map((m) => (
                  <div key={m.model} className="flex items-center justify-between text-sm">
                    <span className="text-muted">{m.model}</span>
                    <span>{m.tokens.toLocaleString()} tokens</span>
                  </div>
                ))}
              </div>
            </div>

            <UserLookup />
          </div>
        )}
      </AdminState>
    </div>
  );
}

function UserLookup() {
  const [query, setQuery] = useState("");
  const [users, setUsers] = useState<AdminUserItem[]>([]);
  const [searched, setSearched] = useState(false);
  const [detail, setDetail] = useState<{ email: string; by_day: { date: string; requests: number; tokens: number }[]; by_endpoint: { endpoint: string; requests: number; tokens: number }[] } | null>(null);

  const searchUsers = async () => {
    const res = await adminListUsers({ search: query, page_size: 10 });
    setUsers(res.items);
    setSearched(true);
    setDetail(null);
  };

  const loadDetail = async (u: AdminUserItem) => {
    const res = await fetch(`/api/admin/analytics/users/${u.id}`);
    if (!res.ok) return;
    setDetail(await res.json());
  };

  return (
    <div className="rounded-xl border border-line bg-surface p-4">
      <h2 className="mb-3 text-sm font-medium">User usage</h2>
      <div className="flex gap-2">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void searchUsers();
          }}
          placeholder="Search user by email"
          aria-label="Search user usage"
          className="rounded-lg border border-line bg-surface-2 px-3 py-1.5 text-sm focus:outline-none"
        />
        <button
          onClick={() => void searchUsers()}
          className="rounded-lg border border-line px-3 py-1.5 text-sm"
        >
          Search
        </button>
      </div>

      {searched && users.length === 0 && (
        <p className="mt-3 text-sm text-faint">No users found.</p>
      )}

      <div className="mt-3 space-y-1">
        {users.map((u) => (
          <button
            key={u.id}
            onClick={() => void loadDetail(u)}
            className="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-sm hover:bg-surface-2"
          >
            <span>{u.email}</span>
            <span className="text-xs text-faint">{u.role}</span>
          </button>
        ))}
      </div>

      {detail && (
        <div className="mt-4 border-t border-line pt-3">
          <h3 className="mb-2 text-sm font-semibold">{detail.email}</h3>
          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <div className="mb-1 text-xs text-faint">Recent days</div>
              <SimpleBar data={detail.by_day.map((d) => ({ date: d.date, value: d.tokens }))} />
            </div>
            <div>
              <div className="mb-1 text-xs text-faint">By endpoint</div>
              <div className="space-y-1">
                {detail.by_endpoint.map((e) => (
                  <div key={e.endpoint} className="flex justify-between text-sm">
                    <span className="text-muted">{e.endpoint}</span>
                    <span>
                      {e.requests} req · {e.tokens.toLocaleString()} tok
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
