"use client";

import { useMemo, useState } from "react";
import {
  adminListUsers,
  adminUpdateUser,
  adminListRoles,
  type AdminUserItem,
} from "@/lib/api";
import { useAdmin, AdminState } from "@/components/admin/useAdmin";

const STATUS_LABEL: Record<string, string> = {
  active: "Active",
  disabled: "Disabled",
  suspended: "Suspended",
};

export default function UsersPage() {
  const [search, setSearch] = useState("");
  const [role, setRole] = useState("");
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);

  const roles = useAdmin(() => adminListRoles(), []);
  const { data, error, loading, reload } = useAdmin(
    () => adminListUsers({ page, page_size: 20, search, role, status }),
    [page, search, role, status]
  );

  const rolesByName = useMemo(
    () => new Map((roles.data?.roles ?? []).map((r) => [r.name, r])),
    [roles.data]
  );

  const [acting, setActing] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const changeRole = async (u: AdminUserItem, roleId: string) => {
    setActing(u.id);
    try {
      await adminUpdateUser(u.id, { role_id: roleId });
      setNotice(`Changed ${u.email} to ${rolesByName.get(roleId)?.name ?? roleId}.`);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to update user.");
    } finally {
      setActing(null);
    }
  };

  const toggleStatus = async (u: AdminUserItem) => {
    setActing(u.id);
    try {
      const next = u.status === "active" ? "disabled" : "active";
      await adminUpdateUser(u.id, { status: next });
      setNotice(`${u.email} is now ${next}.`);
      void reload();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : "Failed to update user.");
    } finally {
      setActing(null);
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <h1 className="mb-4 text-lg font-semibold tracking-tight">Users</h1>

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <input
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            setPage(1);
          }}
          placeholder="Search email or name"
          aria-label="Search users"
          className="rounded-lg border border-line bg-surface px-3 py-1.5 text-sm placeholder:text-faint focus:outline-none"
        />
        <select
          value={role}
          onChange={(e) => {
            setRole(e.target.value);
            setPage(1);
          }}
          aria-label="Filter by role"
          className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm focus:outline-none"
        >
          <option value="">All roles</option>
          {(roles.data?.roles ?? []).map((r) => (
            <option key={r.id} value={r.name}>
              {r.name}
            </option>
          ))}
        </select>
        <select
          value={status}
          onChange={(e) => {
            setStatus(e.target.value);
            setPage(1);
          }}
          aria-label="Filter by status"
          className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm focus:outline-none"
        >
          <option value="">All statuses</option>
          <option value="active">Active</option>
          <option value="disabled">Disabled</option>
        </select>
      </div>

      {notice && (
        <div className="mb-3 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-muted">
          {notice}
        </div>
      )}

      <AdminState loading={loading} error={error}>
        {data && (
          <>
            <div className="overflow-x-auto rounded-xl border border-line bg-surface">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-line text-left text-xs text-faint">
                    <th className="px-3 py-2 font-medium">Email</th>
                    <th className="px-3 py-2 font-medium">Role</th>
                    <th className="px-3 py-2 font-medium">Status</th>
                    <th className="px-3 py-2 font-medium">Last login</th>
                    <th className="px-3 py-2 font-medium">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {data.items.map((u) => (
                    <tr key={u.id} className="border-b border-line last:border-0">
                      <td className="px-3 py-2">
                        <div className="truncate">{u.email}</div>
                        {u.name && <div className="text-xs text-faint">{u.name}</div>}
                      </td>
                      <td className="px-3 py-2">
                        <select
                          value={u.role_id}
                          disabled={acting === u.id}
                          onChange={(e) => void changeRole(u, e.target.value)}
                          aria-label={`Change role for ${u.email}`}
                          className="rounded border border-line bg-surface-2 px-1.5 py-0.5 text-xs"
                        >
                          {(roles.data?.roles ?? []).map((r) => (
                            <option key={r.id} value={r.id}>
                              {r.name}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td className="px-3 py-2">
                        <span
                          className={
                            u.status === "active" ? "text-accent" : "text-muted"
                          }
                        >
                          {STATUS_LABEL[u.status] ?? u.status}
                        </span>
                      </td>
                      <td className="px-3 py-2 text-xs text-faint">
                        {u.last_login_at ? new Date(u.last_login_at).toLocaleString() : "–"}
                      </td>
                      <td className="px-3 py-2">
                        <button
                          onClick={() => void toggleStatus(u)}
                          disabled={acting === u.id}
                          className="text-xs text-muted underline-offset-2 hover:text-foreground hover:underline"
                        >
                          {u.status === "active" ? "Disable" : "Enable"}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <div className="mt-3 flex items-center justify-between text-xs text-faint">
              <span>
                {data.total} users · page {data.page}
              </span>
              <div className="flex gap-2">
                <button
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                  disabled={page <= 1}
                  className="rounded border border-line px-2 py-1 disabled:opacity-40"
                >
                  Prev
                </button>
                <button
                  onClick={() => setPage((p) => p + 1)}
                  disabled={page * data.page_size >= data.total}
                  className="rounded border border-line px-2 py-1 disabled:opacity-40"
                >
                  Next
                </button>
              </div>
            </div>
          </>
        )}
      </AdminState>
    </div>
  );
}
