"use client";

import { Suspense, useEffect, useState, type ReactNode } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { buildDemoScene, DEMO_SESSION_NAME } from "@/lib/demo-graph";
import { seedDemoSession } from "@/lib/persistence";

type DemoState = "working" | "unavailable";

/**
 * Public entry point: mint a demo session, seed the canvas, then hand the
 * visitor to the real app. No credentials, no Google consent screen.
 */
export default function DemoPage() {
  return (
    <Suspense fallback={<DemoShell />}>
      <DemoInner />
    </Suspense>
  );
}

function DemoInner() {
  const search = useSearchParams();
  const next = search.get("next") || "/app";
  const [state, setState] = useState<DemoState>("working");

  useEffect(() => {
    let cancelled = false;

    const enter = async () => {
      // A returning visitor already has a session: skip straight in.
      try {
        const me = await fetch("/auth/me", { credentials: "include" });
        if (me.ok) {
          const body = (await me.json()) as { authenticated?: boolean };
          if (body.authenticated && !cancelled) {
            window.location.replace(next);
            return;
          }
        }
      } catch {
        // Fall through to the demo login.
      }

      try {
        const res = await fetch("/auth/demo/login", {
          method: "POST",
          credentials: "include",
        });
        if (cancelled) return;
        if (!res.ok) {
          setState("unavailable");
          return;
        }
        seedDemoSession(buildDemoScene(), DEMO_SESSION_NAME);
        window.location.replace(next);
      } catch {
        if (!cancelled) setState("unavailable");
      }
    };

    void enter();
    return () => {
      cancelled = true;
    };
  }, [next]);

  return (
    <DemoShell>
      {state === "working" ? (
        <>
          <div className="flex items-center justify-center gap-1.5" aria-hidden>
            {[0, 1, 2].map((i) => (
              <span
                key={i}
                className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent"
                style={{ animationDelay: `${i * 160}ms` }}
              />
            ))}
          </div>
          <p className="mt-6 text-sm text-muted">Preparing your canvas</p>
          <p className="mt-1.5 text-xs text-faint">
            A worked example graph is loading. Nothing to install, nothing to sign in to.
          </p>
        </>
      ) : (
        <>
          <p className="text-sm font-medium">The demo is not available right now.</p>
          <p className="mt-2 text-xs leading-relaxed text-muted">
            This deployment is not accepting demo sessions. You can still sign in and
            use the canvas normally.
          </p>
          <div className="mt-6 flex items-center justify-center gap-3">
            <Link
              href="/login"
              className="rounded-xl bg-accent px-4 py-2 text-sm font-medium text-accent-ink transition-transform active:scale-[0.97]"
            >
              Go to sign in
            </Link>
            <Link
              href="/"
              className="text-sm text-muted underline-offset-4 transition-colors hover:text-foreground hover:underline"
            >
              Back to homepage
            </Link>
          </div>
        </>
      )}
      <noscript>
        <p className="mt-6 text-xs text-muted">
          The demo needs JavaScript. Sign in instead to use the canvas.
        </p>
      </noscript>
    </DemoShell>
  );
}

function DemoShell({ children }: { children?: ReactNode }) {
  return (
    <main className="relative grid min-h-dvh place-items-center overflow-hidden bg-background px-6">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_50%_35%,rgba(124,116,255,0.14),transparent_62%)]"
      />
      <div className="relative w-full max-w-sm rounded-2xl border border-line bg-surface px-8 py-10 text-center">
        <p className="text-lg font-semibold tracking-tight">Weave</p>
        <p className="mt-1 text-xs uppercase tracking-[0.2em] text-faint">Live demo</p>
        <div className="mt-8">{children}</div>
      </div>
    </main>
  );
}
