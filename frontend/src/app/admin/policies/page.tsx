"use client";

import { useState } from "react";
import {
  adminGetPolicies,
  adminPatchRolePolicy,
  adminPatchUserPolicy,
  type Limits,
} from "@/lib/api";
import { useAdmin, AdminState } from "@/components/admin/useAdmin";

const FIELDS: { key: keyof Limits; label: string }[] = [
  { key: "requests_per_minute", label: "Requests/min" },
  { key: "requests_per_hour", label: "Requests/hour" },
  { key: "requests_per_day", label: "Requests/day" },
  { key: "tokens_per_day", label: "Tokens/day" },
  { key: "tokens_per_month", label: "Tokens/month" },
  { key: "concurrent_requests", label: "Concurrent" },
];

export default function PoliciesPage() {
  const { data, error, loading, reload } = useAdmin(() => adminGetPolicies(), []);
  const [editingRole, setEditingRole] = useState<string | null>(null);
  const [draft, setDraft] = useState<Limits>({});
  const [notice, setNotice] = useState<string | null>(null);

  const openRoleEditor = (roleId: string, limits: Limits) => {
    setEditingRole(roleId);
    setDraft(limits);
  };

  const saveRole = async () => {
    if (!editingRole) return;
    try {
      await adminPatchRolePolicy(editingRole, draft);
      setNotice("Role policy saved.");
      setEditingRole(null);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to save.");
    }
  };

  const setUserOverride = async (userId: string, email: string) => {
    const raw = prompt(`Set requests/min override for ${email} (blank removes it):`);
    if (raw === null) return;
    const rpm = raw.trim() === "" ? null : Number(raw);
    if (rpm !== null && (Number.isNaN(rpm) || rpm < 0)) {
      setNotice("Enter a non-negative number.");
      return;
    }
    try {
      await adminPatchUserPolicy(userId, rpm === null ? {} : { requests_per_minute: rpm });
      setNotice(raw.trim() === "" ? "Override removed." : `Override set to ${rpm}/min.`);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to save.");
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <h1 className="mb-4 text-lg font-semibold tracking-tight">Policies</h1>
      {notice && (
        <div className="mb-3 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-muted">
          {notice}
        </div>
      )}

      <AdminState loading={loading} error={error}>
        {data && (
          <div className="space-y-6">
            <section className="rounded-xl border border-line bg-surface p-4">
              <h2 className="mb-3 text-sm font-semibold">Global defaults</h2>
              <div className="grid grid-cols-2 gap-x-6 gap-y-1 sm:grid-cols-3">
                {FIELDS.map((f) => (
                  <div key={f.key} className="flex justify-between text-sm">
                    <span className="text-faint">{f.label}</span>
                    <span>{data.global[f.key]?.toLocaleString() ?? "–"}</span>
                  </div>
                ))}
              </div>
            </section>

            <section className="rounded-xl border border-line bg-surface p-4">
              <h2 className="mb-3 text-sm font-semibold">Role policies</h2>
              <table className="w-full text-sm">
                <tbody>
                  {data.roles.map((r) => (
                    <tr key={r.role_id} className="border-b border-line last:border-0">
                      <td className="py-2 pr-3 font-medium">{r.role}</td>
                      <td className="py-2 text-xs text-faint">
                        {Object.entries(r.limits)
                          .filter(([, v]) => v != null)
                          .map(([k, v]) => `${k.replace(/_/g, " ")} ${v}`)
                          .join(" · ") || "no limits"}
                      </td>
                      <td className="py-2 text-right">
                        {editingRole === r.role_id ? (
                          <span className="flex items-center justify-end gap-2">
                            <button
                              onClick={() => void saveRole()}
                              className="rounded bg-accent px-2 py-1 text-xs font-medium text-accent-ink"
                            >
                              Save
                            </button>
                            <button
                              onClick={() => setEditingRole(null)}
                              className="text-xs text-faint"
                            >
                              Cancel
                            </button>
                          </span>
                        ) : (
                          <button
                            onClick={() => openRoleEditor(r.role_id, r.limits)}
                            className="text-xs text-muted underline-offset-2 hover:text-foreground hover:underline"
                          >
                            Edit
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {editingRole && (
                <div className="mt-3 grid grid-cols-2 gap-3 border-t border-line pt-3 sm:grid-cols-3">
                  {FIELDS.map((f) => (
                    <label key={f.key} className="text-xs text-faint">
                      {f.label}
                      <input
                        type="number"
                        value={draft[f.key] ?? ""}
                        onChange={(e) =>
                          setDraft((d) => ({
                            ...d,
                            [f.key]: e.target.value === "" ? undefined : Number(e.target.value),
                          }))
                        }
                        className="mt-1 w-full rounded border border-line bg-surface-2 px-2 py-1 text-sm text-foreground"
                      />
                    </label>
                  ))}
                </div>
              )}
            </section>

            <section className="rounded-xl border border-line bg-surface p-4">
              <h2 className="mb-3 text-sm font-semibold">User overrides</h2>
              <table className="w-full text-sm">
                <tbody>
                  {data.users.map((u) => (
                    <tr key={u.user_id} className="border-b border-line last:border-0">
                      <td className="py-2 pr-3">
                        {u.email}
                        <span className="ml-2 text-xs text-faint">{u.role}</span>
                      </td>
                      <td className="py-2 text-xs text-faint">
                        {Object.entries(u.overrides)
                          .filter(([, v]) => v != null)
                          .map(([k, v]) => `${k.replace(/_/g, " ")} ${v}`)
                          .join(" · ") || "no override"}
                      </td>
                      <td className="py-2 text-right">
                        <button
                          onClick={() => void setUserOverride(u.user_id, u.email)}
                          className="text-xs text-muted underline-offset-2 hover:text-foreground hover:underline"
                        >
                          {Object.keys(u.overrides).length ? "Edit / Remove" : "Set"}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          </div>
        )}
      </AdminState>
    </div>
  );
}
