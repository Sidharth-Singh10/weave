"use client";

import { useState } from "react";
import { adminOverview } from "@/lib/api";
import { useAdmin, AdminState, Card, SimpleBar } from "@/components/admin/useAdmin";

const RANGES = [7, 30, 90];

export default function OverviewPage() {
  const [days, setDays] = useState(30);
  const { data, error, loading } = useAdmin(() => adminOverview(days), [days]);

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-lg font-semibold tracking-tight">Overview</h1>
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
              {r} days
            </button>
          ))}
        </div>
      </div>

      <AdminState loading={loading} error={error}>
        {data && (
          <>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-6">
              <Card title="Total users" value={data.totals.total_users} />
              <Card title="Active users" value={data.totals.active_users} />
              <Card title="New users" value={data.totals.new_users} sub={`last ${days}d`} />
              <Card title="Requests today" value={data.totals.requests_today} />
              <Card title="LLM tokens today" value={data.totals.llm_tokens_today.toLocaleString()} />
              <Card title="Rate-limit hits" value={data.totals.rate_limit_hits} sub={`last ${days}d`} />
            </div>

            <div className="mt-6 grid gap-4 md:grid-cols-2">
              <Chart title="Active users / day" data={data.charts.active_users} />
              <Chart title="Requests / day" data={data.charts.requests} />
              <Chart title="LLM tokens / day" data={data.charts.llm_tokens} />
              <Chart title="New users / day" data={data.charts.new_users} />
            </div>

            <div className="mt-6 grid gap-4 md:grid-cols-2">
              <div className="rounded-xl border border-line bg-surface p-4">
                <h2 className="mb-2 text-sm font-medium">LLM usage</h2>
                <p className="text-xs text-faint">
                  {data.llm.input_tokens.toLocaleString()} in · {data.llm.output_tokens.toLocaleString()} out ·{" "}
                  {data.llm.total_tokens.toLocaleString()} total
                </p>
                <div className="mt-3 space-y-1">
                  {data.llm.by_model.map((m) => (
                    <div key={m.model} className="flex justify-between text-sm">
                      <span className="text-muted">{m.model}</span>
                      <span>{m.tokens.toLocaleString()}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="rounded-xl border border-line bg-surface p-4">
                <h2 className="mb-2 text-sm font-medium">Latency</h2>
                <p className="text-sm text-muted">
                  avg {data.api.avg_latency_ms?.toFixed(0) ?? "–"} ms · p95{" "}
                  {data.api.p95_latency_ms?.toFixed(0) ?? "–"} ms
                </p>
                <h2 className="mb-2 mt-4 text-sm font-medium">Top users by tokens</h2>
                <div className="space-y-1">
                  {data.top_users.map((u) => (
                    <div key={u.email} className="flex justify-between text-sm">
                      <span className="truncate text-muted">{u.email}</span>
                      <span>{u.tokens.toLocaleString()}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </>
        )}
      </AdminState>
    </div>
  );
}

function Chart({ title, data }: { title: string; data: { date: string; value: number }[] }) {
  return (
    <div className="rounded-xl border border-line bg-surface p-4">
      <h2 className="mb-3 text-sm font-medium">{title}</h2>
      <SimpleBar data={data} />
    </div>
  );
}
