"use client";

import { useEffect } from "react";
import { useAuthStore, redirectToLogin } from "@/lib/auth";

/**
 * Client-side auth guard. The server (proxy.ts + backend /auth/me) is
 * authoritative; this only renders the app once the session is confirmed and
 * redirects to /login when it isn't. Avoids infinite loops because /login is
 * not guarded.
 */
export function RequireAuth({ children }: { children: React.ReactNode }) {
  const status = useAuthStore((s) => s.status);

  useEffect(() => {
    if (status === "loading") {
      void useAuthStore.getState().bootstrap();
    }
  }, [status]);

  if (status === "loading") {
    return (
      <div className="grid min-h-dvh place-items-center text-sm text-faint">
        Loading…
      </div>
    );
  }

  if (status === "unauthenticated") {
    // Render a redirect during the effect cycle to avoid a setState-in-render.
    return <RedirectToLogin />;
  }

  return <>{children}</>;
}

function RedirectToLogin() {
  useEffect(() => {
    redirectToLogin();
  }, []);
  return null;
}
