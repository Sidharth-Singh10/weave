"use client";

import { useState } from "react";
import {
  adminListRoles,
  adminCreateRole,
  adminUpdateRole,
  adminDeleteRole,
  type AdminRole,
} from "@/lib/api";
import { useAdmin, AdminState } from "@/components/admin/useAdmin";

const ALL_PERMISSIONS = [
  "admin.users.read",
  "admin.users.update",
  "admin.roles.read",
  "admin.roles.update",
  "admin.policies.read",
  "admin.policies.update",
  "admin.analytics.read",
  "admin.audit.read",
  "graph.ingest",
  "graph.organize",
  "graph.label_community",
  "graph.search",
];

export default function RolesPage() {
  const { data, error, loading, reload } = useAdmin(() => adminListRoles(), []);
  const [selected, setSelected] = useState<AdminRole | null>(null);
  const [draft, setDraft] = useState<Set<string>>(new Set());
  const [notice, setNotice] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  const openRole = (r: AdminRole) => {
    setSelected(r);
    setDraft(new Set(r.permissions));
  };

  const saveRole = async () => {
    if (!selected) return;
    try {
      await adminUpdateRole(selected.id, {
        permission_keys: Array.from(draft),
      });
      setNotice(`Saved ${selected.name} permissions.`);
      setSelected(null);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to save.");
    }
  };

  const createRole = async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      await adminCreateRole({ name, permission_keys: [] });
      setNotice(`Created role "${name}".`);
      setNewName("");
      setCreating(false);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to create.");
    }
  };

  const deleteRole = async (r: AdminRole) => {
    if (!confirm(`Delete role "${r.name}"?`)) return;
    try {
      await adminDeleteRole(r.id);
      setNotice(`Deleted ${r.name}.`);
      if (selected?.id === r.id) setSelected(null);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to delete.");
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-lg font-semibold tracking-tight">Roles</h1>
        <button
          onClick={() => setCreating((v) => !v)}
          className="rounded-lg bg-accent px-3 py-1.5 text-sm font-medium text-accent-ink"
        >
          New role
        </button>
      </div>

      {creating && (
        <div className="mb-4 flex gap-2">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Role name"
            aria-label="New role name"
            className="rounded-lg border border-line bg-surface px-3 py-1.5 text-sm focus:outline-none"
          />
          <button
            onClick={() => void createRole()}
            className="rounded-lg border border-line px-3 py-1.5 text-sm"
          >
            Create
          </button>
        </div>
      )}

      {notice && (
        <div className="mb-3 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-muted">
          {notice}
        </div>
      )}

      <AdminState loading={loading} error={error}>
        {data && (
          <div className="grid gap-4 md:grid-cols-2">
            <div className="rounded-xl border border-line bg-surface">
              {(data.roles ?? []).map((r) => (
                <button
                  key={r.id}
                  onClick={() => openRole(r)}
                  className={[
                    "flex w-full items-center justify-between border-b border-line px-4 py-3 text-left last:border-0",
                    selected?.id === r.id ? "bg-surface-2" : "hover:bg-surface-2/60",
                  ].join(" ")}
                >
                  <span className="font-medium">{r.name}</span>
                  <span className="text-xs text-faint">
                    {r.permissions.length} permissions
                  </span>
                </button>
              ))}
            </div>

            {selected ? (
              <div className="rounded-xl border border-line bg-surface p-4">
                <div className="mb-3 flex items-center justify-between">
                  <h2 className="font-semibold">{selected.name}</h2>
                  <div className="flex gap-2">
                    <button
                      onClick={() => setSelected(null)}
                      className="text-xs text-faint hover:text-muted"
                    >
                      Close
                    </button>
                    <button
                      onClick={() => void deleteRole(selected)}
                      className="text-xs text-muted hover:text-foreground"
                    >
                      Delete
                    </button>
                  </div>
                </div>
                <div className="grid gap-1">
                  {ALL_PERMISSIONS.map((p) => (
                    <label key={p} className="flex cursor-pointer items-center gap-2 text-sm text-muted">
                      <input
                        type="checkbox"
                        checked={draft.has(p)}
                        onChange={() =>
                          setDraft((prev) => {
                            const next = new Set(prev);
                            if (next.has(p)) next.delete(p);
                            else next.add(p);
                            return next;
                          })
                        }
                      />
                      {p}
                    </label>
                  ))}
                </div>
                <button
                  onClick={() => void saveRole()}
                  className="mt-4 rounded-lg bg-accent px-3 py-1.5 text-sm font-medium text-accent-ink"
                >
                  Save permissions
                </button>
              </div>
            ) : (
              <div className="rounded-xl border border-line bg-surface p-6 text-sm text-faint">
                Select a role to edit its permissions.
              </div>
            )}
          </div>
        )}
      </AdminState>
    </div>
  );
}
