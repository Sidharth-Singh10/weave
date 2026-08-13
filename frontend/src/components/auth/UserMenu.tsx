"use client";

import { useState } from "react";
import Link from "next/link";
import { SignOut } from "@phosphor-icons/react";
import { useAuthStore, isAdmin } from "@/lib/auth";

/** Authenticated user menu: avatar, name/email, account actions. */
export function UserMenu() {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const [open, setOpen] = useState(false);

  if (!user) return null;

  const initials = (user.name || user.email)
    .split(/\s+/)
    .map((p) => p[0]?.toUpperCase())
    .slice(0, 2)
    .join("");

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Account menu"
        className="grid size-7 place-items-center rounded-full border border-line bg-surface-2 text-xs font-medium text-foreground"
      >
        {user.avatar_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={user.avatar_url}
            alt=""
            className="size-7 rounded-full"
            referrerPolicy="no-referrer"
          />
        ) : (
          initials || "?"
        )}
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-20" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full z-30 mt-2 w-56 rounded-xl border border-line bg-surface p-1 shadow-lg">
            <div className="px-3 py-2">
              <div className="truncate text-sm font-medium text-foreground">
                {user.name || "Account"}
              </div>
              <div className="truncate text-xs text-faint">{user.email}</div>
            </div>
            <div className="my-1 border-t border-line" />
            {isAdmin(user) && (
              <Link
                href="/admin"
                onClick={() => setOpen(false)}
                className="block rounded-lg px-3 py-2 text-sm text-muted hover:bg-surface-2 hover:text-foreground"
              >
                Admin Dashboard
              </Link>
            )}
            <button
              onClick={() => void logout()}
              className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm text-muted hover:bg-surface-2 hover:text-foreground"
            >
              <SignOut size={14} />
              Logout
            </button>
          </div>
        </>
      )}
    </div>
  );
}
