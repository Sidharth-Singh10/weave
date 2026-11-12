"use client";

import { useEffect } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useAuthStore, redirectToLogin } from "@/lib/auth";
import { RequireAuth } from "./RequireAuth";
import { Logo } from "@/components/ui/Logo";

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

const LINKS = [
  { href: "/admin", label: "Overview" },
  { href: "/admin/users", label: "Users" },
  { href: "/admin/roles", label: "Roles" },
  { href: "/admin/policies", label: "Policies" },
  { href: "/admin/pipeline", label: "Pipeline" },
  { href: "/admin/analytics", label: "Analytics" },
  { href: "/admin/audit", label: "Audit Log" },
];

export function AdminNav() {
  const user = useAuthStore((s) => s.user);
  const pathname = usePathname();

  return (
    <aside className="flex shrink-0 flex-col border-b border-line bg-surface md:min-h-dvh md:w-52 md:border-b-0 md:border-r">
      <div className="flex items-center justify-between gap-2 px-4 py-3">
        <Link href="/admin" aria-label="Admin overview" className="rounded-lg">
          <Logo />
        </Link>
        <Link
          href="/app"
          className="rounded-lg border border-line px-2 py-1 font-mono text-[10px] uppercase tracking-[0.12em] text-faint transition-colors hover:text-foreground md:hidden"
        >
          Canvas
        </Link>
      </div>

      <nav
        aria-label="Admin sections"
        className="flex gap-1 overflow-x-auto px-2 pb-2 md:mt-1 md:flex-col md:overflow-visible md:px-2 md:pb-0"
      >
        <p className="hidden px-2 pb-2 pt-1 font-mono text-[10px] uppercase tracking-[0.16em] text-faint md:block">
          Admin
        </p>
        {LINKS.map((link) => {
          const active = pathname === link.href;
          return (
            <Link
              key={link.href}
              href={link.href}
              aria-current={active ? "page" : undefined}
              className={[
                "shrink-0 whitespace-nowrap rounded-xl px-3 py-2 text-sm transition-colors md:px-2.5",
                active
                  ? "bg-accent/[0.1] text-accent"
                  : "text-muted hover:bg-surface-2 hover:text-foreground",
              ].join(" ")}
            >
              {link.label}
            </Link>
          );
        })}
      </nav>

      <div className="mt-auto hidden border-t border-line p-4 md:block">
        <div className="truncate px-1 text-[11px] text-faint">{user?.email}</div>
        <Link
          href="/app"
          className="mt-2 block px-1 text-xs text-muted transition-colors hover:text-foreground"
        >
          Back to canvas
        </Link>
      </div>
    </aside>
  );
}
