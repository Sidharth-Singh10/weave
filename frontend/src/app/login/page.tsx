"use client";

import { Suspense, useEffect } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { useAuthStore } from "@/lib/auth";

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
      <div className="w-full max-w-sm rounded-2xl border border-line bg-surface p-8 text-center">
        <p className="text-lg font-semibold tracking-tight">Weave</p>
      </div>
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
    <main className="grid min-h-dvh place-items-center bg-background">
      <div className="w-full max-w-sm rounded-2xl border border-line bg-surface p-8 text-center">
        <p className="text-lg font-semibold tracking-tight">Weave</p>
        <p className="mt-1 text-sm text-muted">Think in graphs.</p>

        <div className="mt-8 flex flex-col gap-2">
          <a
            href="/auth/google"
            className="rounded-xl bg-accent px-4 py-2.5 text-sm font-medium text-accent-ink transition-transform active:scale-[0.97]"
          >
            Continue with Google
          </a>
          <button
            onClick={handleTestLogin}
            className="rounded-xl border border-line px-4 py-2 text-xs text-muted transition-colors hover:bg-surface-2"
          >
            Continue with a test account
          </button>
        </div>

        {error && ERROR_MESSAGES[error] && (
          <p className="mt-5 rounded-lg border border-line bg-background px-3 py-2 text-sm text-muted">
            {ERROR_MESSAGES[error]}
          </p>
        )}

        <p className="mt-8 text-xs text-faint">
          <Link href="/" className="hover:text-muted">
            Back to homepage
          </Link>
        </p>
      </div>
    </main>
  );
}
