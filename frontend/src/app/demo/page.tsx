"use client";

import { Suspense, useEffect, useState, type ReactNode } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { buildDemoScene, DEMO_SESSION_NAME } from "@/lib/demo-graph";
import { seedDemoSession } from "@/lib/persistence";
import { Logo } from "@/components/ui/Logo";

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
        <div role="status" aria-live="polite">
          <div className="flex items-center justify-center gap-1.5" aria-hidden>
            {[0, 1, 2].map((i) => (
              <span
                key={i}
                className="size-1.5 animate-pulse rounded-full bg-accent"
                style={{ animationDelay: `${i * 160}ms` }}
              />
            ))}
          </div>
          <p className="mt-6 text-sm text-muted">Preparing your canvas</p>
          <p className="mt-1.5 text-xs leading-relaxed text-faint">
            A worked example graph is loading. Nothing to install, nothing to
            sign in to.
          </p>
        </div>
      ) : (
        <div role="alert">
          <p className="text-sm font-medium">The demo is not available right now.</p>
          <p className="mt-2 text-xs leading-relaxed text-muted">
            This deployment is not accepting demo sessions. You can still sign in
            and use the canvas normally.
          </p>
          <div className="mt-6 flex items-center justify-center gap-3">
            <Link
              href="/login"
              className="rounded-xl bg-accent px-4 py-2 text-sm font-medium text-accent-ink transition-colors hover:bg-accent-bright active:scale-[0.98]"
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
        </div>
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
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div className="weave-grid absolute inset-0 opacity-[0.1] [mask-image:radial-gradient(ellipse_at_50%_40%,black,transparent_72%)]" />
        <div className="absolute left-1/2 top-0 size-[520px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent/[0.07] blur-[120px]" />
      </div>

      <div className="relative w-full max-w-md rounded-2xl border border-line bg-surface px-8 py-10 text-center shadow-panel">
        <div className="flex justify-center">
          <Logo />
        </div>
        <p className="mt-2 font-mono text-[10px] uppercase tracking-[0.18em] text-faint">
          Live demo
        </p>
        <div className="mt-8">{children}</div>
        <p className="mt-8 border-t border-line pt-4 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
          No account. No install.
        </p>
      </div>
    </main>
  );
}
