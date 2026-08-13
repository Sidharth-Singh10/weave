"use client";

import { useEffect, useState } from "react";
import { ApiError } from "@/lib/api";

export function useAdmin<T>(fn: () => Promise<T>, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    fn()
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.status === 403) {
          setError("You don't have permission to view this.");
        } else if (e instanceof ApiError && e.status === 401) {
          setError("Your session expired. Please sign in again.");
        } else {
          setError(e instanceof Error ? e.message : "Failed to load.");
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);

  const reload = () => {
    setLoading(true);
    setError(null);
    setTick((t) => t + 1);
  };

  return { data, error, loading, reload };
}

export function AdminState({ loading, error, children }: { loading: boolean; error: string | null; children: React.ReactNode }) {
  if (loading) return <p className="text-sm text-faint">Loading…</p>;
  if (error) {
    return (
      <div className="rounded-lg border border-line bg-surface px-4 py-3 text-sm text-muted">
        {error}
      </div>
    );
  }
  return <>{children}</>;
}

export function Card({ title, value, sub }: { title: string; value: React.ReactNode; sub?: string }) {
  return (
    <div className="rounded-xl border border-line bg-surface p-4">
      <div className="text-xs text-faint">{title}</div>
      <div className="mt-1 text-2xl font-semibold tracking-tight">{value}</div>
      {sub && <div className="mt-0.5 text-xs text-faint">{sub}</div>}
    </div>
  );
}

export function SimpleBar({ data }: { data: { date: string; value: number }[] }) {
  const max = Math.max(1, ...data.map((d) => d.value));
  return (
    <div className="flex h-24 items-end gap-[2px]">
      {data.map((d) => (
        <div key={d.date} className="flex-1">
          <div
            className="w-full rounded-t bg-accent/70"
            style={{ height: `${(d.value / max) * 100}%` }}
            title={`${d.date}: ${d.value}`}
          />
        </div>
      ))}
    </div>
  );
}
