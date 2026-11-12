"use client";

import Link from "next/link";
import { GithubLogo } from "@phosphor-icons/react";
import { canvasEntryHref, demoModeEnabled } from "@/lib/demo-mode";
import { ButtonLink } from "@/components/ui/Button";
import { Logo } from "@/components/ui/Logo";

const FOOTER_LINKS = [
  { href: "#how", label: "How it works" },
  { href: "#canvas", label: "Canvas" },
  { href: "#memory", label: "Memory" },
  { href: "#capabilities", label: "Capabilities" },
  { href: "#uses", label: "Use cases" },
];

export function CtaFooter() {
  return (
    <>
      <section className="border-t border-line/70">
        <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
          <div className="relative overflow-hidden rounded-2xl border border-line bg-surface px-6 py-14 text-center shadow-panel md:py-20">
            <div
              aria-hidden
              className="pointer-events-none absolute inset-0"
            >
              <div className="weave-grid absolute inset-0 opacity-[0.14] [mask-image:radial-gradient(ellipse_at_50%_40%,black,transparent_70%)]" />
              <div className="absolute left-1/2 top-0 size-[420px] -translate-x-1/2 -translate-y-1/3 rounded-full bg-accent/[0.09] blur-[100px]" />
            </div>

            <div className="relative">
              <h2 className="mx-auto max-w-2xl text-balance font-display text-3xl font-semibold leading-[1.02] tracking-[-0.045em] md:text-5xl">
                Stop organizing. Start understanding.
              </h2>
              <p className="mx-auto mt-5 max-w-md text-pretty text-sm leading-relaxed text-muted md:text-base">
                Your next note is one sentence away from becoming a map.
              </p>
              <div className="mt-9 flex flex-col items-center gap-3">
                <ButtonLink href={canvasEntryHref} size="lg">
                  Open the canvas
                </ButtonLink>
                <p className="text-xs text-faint">
                  {demoModeEnabled
                    ? "The demo opens a seeded graph. No sign-in required."
                    : "Sign in with Google and start with one note."}
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <footer className="border-t border-line/70">
        <div className="mx-auto grid max-w-6xl grid-cols-1 gap-8 px-6 py-10 md:grid-cols-[1.2fr_0.8fr_0.8fr]">
          <div>
            <Logo />
            <p className="mt-3 max-w-xs text-xs leading-relaxed text-faint">
              Notes in, a living knowledge graph out. Client-owned on the canvas,
              server-owned for agents.
            </p>
            <p className="mt-4 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              Type it. See it. Know it.
            </p>
          </div>

          <nav aria-label="Footer sections">
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              Explore
            </p>
            <ul className="mt-3 space-y-1.5">
              {FOOTER_LINKS.map((link) => (
                <li key={link.href}>
                  <a
                    href={link.href}
                    className="text-xs text-muted transition-colors hover:text-foreground"
                  >
                    {link.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>

          <nav aria-label="Project links">
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              Project
            </p>
            <ul className="mt-3 space-y-1.5">
              <li>
                <Link
                  href="/login"
                  className="text-xs text-muted transition-colors hover:text-foreground"
                >
                  Sign in
                </Link>
              </li>
              <li>
                <a
                  href="https://github.com/Sidharth-Singh10/weave"
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-1.5 text-xs text-muted transition-colors hover:text-foreground"
                >
                  <GithubLogo size={12} />
                  GitHub
                </a>
              </li>
              <li>
                <a
                  href="https://github.com/Sidharth-Singh10/weave#readme"
                  target="_blank"
                  rel="noreferrer"
                  className="text-xs text-muted transition-colors hover:text-foreground"
                >
                  README
                </a>
              </li>
            </ul>
          </nav>
        </div>

        <div className="border-t border-line/70">
          <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-2 px-6 py-4 text-[11px] text-faint sm:flex-row">
            <span>
              {new Date().getFullYear()} Weave. Built with Next.js, Rust,
              PostgreSQL, Redis, and pgvector.
            </span>
            <span className="font-mono uppercase tracking-[0.14em]">
              One graph, many projections
            </span>
          </div>
        </div>
      </footer>
    </>
  );
}
