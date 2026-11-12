"use client";

import { useReducedMotion } from "motion/react";
import { USE_CASES, USE_CASE_DOMAINS } from "@/lib/landing-content";
import { SectionHeading } from "@/components/ui/SectionHeading";

export function UseCases() {
  const reduce = useReducedMotion();
  const row = [...USE_CASE_DOMAINS, ...USE_CASE_DOMAINS];

  return (
    <section id="uses" className="overflow-hidden border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <SectionHeading
          title="One instrument, many subjects."
          lead="Weave knows nothing about your domain, and that is the point. If you can write a sentence about it, it belongs on the graph."
        />

        <div className="mt-12 grid grid-cols-1 gap-px border border-line bg-line md:grid-cols-2">
          {USE_CASES.map((useCase) => (
            <div
              key={useCase.index}
              className="group bg-background p-5 transition-colors hover:bg-surface/60"
            >
              <div className="flex items-center justify-between gap-3">
                <span className="font-mono text-[10px] tracking-[0.16em] text-faint">
                  {useCase.index}
                </span>
                <span className="rounded-full border border-line px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.12em] text-faint transition-colors group-hover:border-line-bright group-hover:text-muted">
                  {useCase.tag}
                </span>
              </div>
              <h3 className="mt-3 text-base font-medium tracking-tight text-foreground">
                {useCase.title}
              </h3>
              <p className="mt-1.5 max-w-md text-sm leading-relaxed text-muted">
                {useCase.scenario}
              </p>
            </div>
          ))}
        </div>
      </div>

      <div className="relative border-t border-line/70 py-7">
        <div className="pointer-events-none absolute inset-y-0 left-0 z-10 w-24 bg-gradient-to-r from-background to-transparent" />
        <div className="pointer-events-none absolute inset-y-0 right-0 z-10 w-24 bg-gradient-to-l from-background to-transparent" />
        <div
          className={`flex w-max gap-3 ${reduce ? "" : "animate-[weave-marquee_38s_linear_infinite]"}`}
        >
          {row.map((domain, index) => (
            <span
              key={`${domain}-${index}`}
              className="whitespace-nowrap rounded-xl border border-line bg-surface px-5 py-2.5 font-mono text-[11px] uppercase tracking-[0.14em] text-faint"
            >
              {domain}
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}
