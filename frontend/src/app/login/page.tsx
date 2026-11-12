"use client";

import { Suspense, useEffect } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { CheckCircle } from "@phosphor-icons/react";
import { useAuthStore } from "@/lib/auth";
import { CANVAS_GRAPH } from "@/lib/landing-content";
import { Logo } from "@/components/ui/Logo";
import { GraphStage } from "@/components/landing/GraphStage";

const ERROR_MESSAGES: Record<string, string> = {
  disabled: "Your account has been disabled.",
  denied: "Sign in was cancelled.",
  invalid_state: "That sign-in link was invalid or expired. Please try again.",
  auth_failed: "Sign-in could not be completed. Please try again.",
  service_unavailable: "Sign-in is temporarily unavailable. Please try again later.",
  rate_limited: "Too many sign-in attempts. Please wait a moment and try again.",
};

export default function LoginPage() {
  return (
    <Suspense fallback={<LoginShell />}>
      <LoginInner />
    </Suspense>
  );
}

function LoginShell() {
  return (
    <main className="grid min-h-dvh place-items-center bg-background">
      <Logo />
    </main>
  );
}

function LoginInner() {
  const status = useAuthStore((s) => s.status);
  const search = useSearchParams();
  const next = search.get("next") || "/app";
  const error = search.get("error");

  useEffect(() => {
    if (status === "loading") {
      void useAuthStore.getState().bootstrap();
    }
    if (status === "authenticated") {
      window.location.href = next;
    }
  }, [status, next]);

  const handleTestLogin = async () => {
    const res = await fetch("/auth/test/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ email: "admin@example.com" }),
    });
    if (res.ok) {
      window.location.href = next;
    }
  };

  return (
    <main className="relative grid min-h-dvh place-items-center overflow-hidden bg-background px-4 py-10 sm:px-6">
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div className="weave-grid absolute inset-0 opacity-[0.1] [mask-image:radial-gradient(ellipse_at_50%_40%,black,transparent_72%)]" />
        <div className="absolute left-1/2 top-0 size-[540px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent/[0.06] blur-[120px]" />
      </div>

      <div className="relative grid w-full max-w-4xl overflow-hidden rounded-2xl border border-line bg-surface shadow-panel md:grid-cols-[1.05fr_1fr]">
        <div className="relative hidden flex-col justify-between border-r border-line bg-surface-2/40 p-8 md:flex">
          <Logo />

          <div className="relative mt-6 h-52 overflow-hidden rounded-xl border border-line bg-background/50">
            <GraphStage
              nodes={CANVAS_GRAPH.nodes}
              edges={CANVAS_GRAPH.edges}
              level={0}
              label="A small knowledge graph"
            />
            <span className="absolute left-3 top-3 font-mono text-[9px] uppercase tracking-[0.14em] text-faint">
              canvas preview
            </span>
          </div>

          <div className="mt-6">
            <p className="font-display text-3xl font-semibold leading-[1.04] tracking-[-0.04em]">
              Notes in. A living graph out.
            </p>
            <p className="mt-3 max-w-xs text-sm leading-relaxed text-muted">
              Your graph stays in this browser. The API handles identity,
              permissions, limits, and extraction, and never persists the graph.
            </p>
            <ul className="mt-5 space-y-2">
              {[
                "Server-side sessions, one origin",
                "Every edge keeps its source sentence",
                "Export the whole graph at any time",
              ].map((item) => (
                <li key={item} className="flex items-start gap-2 text-xs text-muted">
                  <CheckCircle size={14} className="mt-0.5 shrink-0 text-accent" />
                  {item}
                </li>
              ))}
            </ul>
          </div>

          <p className="mt-6 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
            Weave / sign in
          </p>
        </div>

        <div className="flex flex-col justify-center p-8 sm:p-10">
          <div className="md:hidden">
            <Logo />
          </div>

          <h1 className="mt-8 font-display text-2xl font-semibold tracking-[-0.04em] md:mt-0">
            Sign in to Weave
          </h1>
          <p className="mt-2 text-sm leading-relaxed text-muted">
            Use your Google account to open the canvas. Your session is stored in
            an HttpOnly cookie.
          </p>

          <div className="mt-8 flex flex-col gap-2">
            <a
              href="/auth/google"
              className="inline-flex items-center justify-center rounded-xl bg-accent px-4 py-2.5 text-sm font-medium text-accent-ink transition-colors hover:bg-accent-bright active:scale-[0.99]"
            >
              Continue with Google
            </a>
            <button
              type="button"
              onClick={() => void handleTestLogin()}
              className="rounded-xl border border-line bg-surface px-4 py-2 text-xs text-muted transition-colors hover:border-line-bright hover:bg-surface-2 hover:text-foreground"
            >
              Continue with a test account
            </button>
          </div>

          {error && ERROR_MESSAGES[error] && (
            <p
              role="alert"
              className="mt-5 rounded-xl border border-warning/25 bg-warning/[0.06] px-3 py-2 text-sm text-muted"
            >
              {ERROR_MESSAGES[error]}
            </p>
          )}

          <p className="mt-8 text-xs text-faint">
            <Link href="/" className="transition-colors hover:text-muted">
              Back to homepage
            </Link>
          </p>
        </div>
      </div>
    </main>
  );
}
