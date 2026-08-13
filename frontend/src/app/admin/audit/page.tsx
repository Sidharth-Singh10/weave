"use client";

import { useState } from "react";
import { adminAudit, type AdminAuditItem } from "@/lib/api";
import { useAdmin, AdminState } from "@/components/admin/useAdmin";

const ACTIONS = [
  "",
  "user.role_changed",
  "user.disabled",
  "user.enabled",
  "role.created",
  "role.updated",
  "role.deleted",
  "role.permissions_changed",
  "policy.role_updated",
  "policy.user_override_updated",
  "policy.user_override_removed",
];

export default function AuditPage() {
  const [action, setAction] = useState("");
  const [cursor, setCursor] = useState<string | null>(null);
  const { data, error, loading } = useAdmin(
    () => adminAudit({ limit: 50, action: action || undefined, cursor: cursor ?? undefined }),
    [action, cursor]
  );

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-lg font-semibold tracking-tight">Audit Log</h1>
        <select
          value={action}
          onChange={(e) => {
            setAction(e.target.value);
            setCursor(null);
          }}
          aria-label="Filter by action"
          className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm focus:outline-none"
        >
          {ACTIONS.map((a) => (
            <option key={a} value={a}>
              {a === "" ? "All actions" : a}
            </option>
          ))}
        </select>
      </div>

      <AdminState loading={loading} error={error}>
        {data && (
          <>
            <div className="rounded-xl border border-line bg-surface">
              {(data.items ?? []).map((e) => (
                <AuditRow key={e.id} item={e} />
              ))}
              {(data.items ?? []).length === 0 && (
                <p className="px-4 py-6 text-sm text-faint">No audit entries.</p>
              )}
            </div>
            <div className="mt-3">
              <button
                onClick={() => setCursor(data.next_cursor)}
                disabled={!data.next_cursor}
                className="rounded border border-line px-3 py-1.5 text-sm disabled:opacity-40"
              >
                Load more
              </button>
            </div>
          </>
        )}
      </AdminState>
    </div>
  );
}

function AuditRow({ item }: { item: AdminAuditItem }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="border-b border-line last:border-0">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center justify-between px-4 py-2.5 text-left hover:bg-surface-2/60"
      >
        <span className="text-sm">
          <span className="font-mono text-xs text-accent">{item.action}</span>
          <span className="ml-3 text-muted">{item.actor_email ?? "–"}</span>
        </span>
        <span className="text-xs text-faint">
          {new Date(item.created_at).toLocaleString()}
        </span>
      </button>
      {open && (
        <pre className="overflow-x-auto px-4 pb-3 text-xs text-faint">
          {JSON.stringify(
            { target_type: item.target_type, target_id: item.target_id, old: item.old_value, new: item.new_value },
            null,
            2
          )}
        </pre>
      )}
    </div>
  );
}
