"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { SignOut } from "@phosphor-icons/react";
import { useAuthStore, isAdmin } from "@/lib/auth";

/** Authenticated user menu: avatar, name/email, account actions. */
export function UserMenu() {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);

  if (!user) return null;

  const initials = (user.name || user.email)
    .split(/\s+/)
    .map((part) => part[0]?.toUpperCase())
    .slice(0, 2)
    .join("");

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-label="Account menu"
        aria-haspopup="menu"
        aria-expanded={open}
        className="grid size-8 place-items-center overflow-hidden rounded-full border border-line bg-surface-2 text-xs font-medium text-foreground transition-colors hover:border-line-bright"
      >
        {user.avatar_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={user.avatar_url}
            alt=""
            className="size-8 rounded-full"
            referrerPolicy="no-referrer"
          />
        ) : (
          initials || "?"
        )}
      </button>

      {open && (
        <>
          <div
            className="fixed inset-0 z-20 bg-background/20"
            onClick={() => setOpen(false)}
            aria-hidden
          />
          <div
            role="menu"
            aria-label="Account"
            className="absolute right-0 top-full z-30 mt-2 w-60 rounded-2xl border border-line bg-surface p-1 shadow-float"
          >
            <div className="px-3 py-2.5">
              <div className="truncate text-sm font-medium text-foreground">
                {user.name || "Account"}
              </div>
              <div className="truncate text-xs text-faint">{user.email}</div>
            </div>
            <div className="my-1 border-t border-line" />
            {isAdmin(user) && (
              <Link
                role="menuitem"
                href="/admin"
                onClick={() => setOpen(false)}
                className="block rounded-xl px-3 py-2 text-sm text-muted transition-colors hover:bg-surface-2 hover:text-foreground"
              >
                Admin Dashboard
              </Link>
            )}
            <button
              type="button"
              role="menuitem"
              onClick={() => void logout()}
              className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-sm text-muted transition-colors hover:bg-surface-2 hover:text-foreground"
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
