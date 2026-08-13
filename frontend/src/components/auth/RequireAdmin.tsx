"use client";

import { useEffect } from "react";
import Link from "next/link";
import { useAuthStore, redirectToLogin } from "@/lib/auth";
import { RequireAuth } from "./RequireAuth";

/**
 * Guard for the /admin area: requires an authenticated session AND the admin
 * role. The backend still enforces permissions on every admin route.
 */
export function RequireAdmin({ children }: { children: React.ReactNode }) {
  return (
    <RequireAuth>
      <AdminCheck>{children}</AdminCheck>
    </RequireAuth>
  );
}

function AdminCheck({ children }: { children: React.ReactNode }) {
  const user = useAuthStore((s) => s.user);

  useEffect(() => {
    if (user && user.role !== "admin") {
      redirectToLogin();
    }
  }, [user]);

  if (!user) return null;
  if (user.role !== "admin") return null;

  return <>{children}</>;
}

export function AdminNav() {
  const user = useAuthStore((s) => s.user);
  const links = [
    { href: "/admin", label: "Overview" },
    { href: "/admin/users", label: "Users" },
    { href: "/admin/roles", label: "Roles" },
    { href: "/admin/policies", label: "Policies" },
    { href: "/admin/analytics", label: "Analytics" },
    { href: "/admin/audit", label: "Audit Log" },
  ];
  return (
    <aside className="flex w-44 shrink-0 flex-col gap-1 border-r border-line bg-surface p-3">
      <Link href="/admin" className="mb-3 px-2 text-sm font-semibold tracking-tight">
        Admin
      </Link>
      {links.map((l) => (
        <Link
          key={l.href}
          href={l.href}
          className="rounded-md px-2 py-1.5 text-sm text-muted transition-colors hover:bg-surface-2 hover:text-foreground"
        >
          {l.label}
        </Link>
      ))}
      <div className="mt-auto border-t border-line pt-3">
        <div className="px-2 text-xs text-faint">{user?.email}</div>
        <Link href="/app" className="mt-1 block px-2 text-xs text-muted hover:text-foreground">
          Back to canvas
        </Link>
      </div>
    </aside>
  );
}
