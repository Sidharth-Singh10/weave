"use client";

import { useReducedMotion } from "motion/react";

const domains = [
  "Novels & characters",
  "Academic learning",
  "Programming concepts",
  "Medical education",
  "Law",
  "History",
  "Scientific research",
  "Personal knowledge",
  "Project planning",
  "Worldbuilding",
];

export function UseCases() {
  const reduce = useReducedMotion();
  const row = [...domains, ...domains];

  return (
    <section id="uses" className="overflow-hidden border-t border-line/60">
      <div className="mx-auto max-w-6xl px-6 py-24 md:py-32">
        <h2 className="max-w-md text-3xl font-semibold tracking-tighter md:text-4xl">
          One tool, every subject
        </h2>
        <p className="mt-4 max-w-xl text-base leading-relaxed text-muted">
          Weave knows nothing about your domain, and that is the point. If you
          can write a sentence about it, it belongs on the graph.
        </p>
      </div>

      <div className="relative border-t border-line/60 py-8">
        <div className="pointer-events-none absolute inset-y-0 left-0 z-10 w-24 bg-gradient-to-r from-background to-transparent" />
        <div className="pointer-events-none absolute inset-y-0 right-0 z-10 w-24 bg-gradient-to-l from-background to-transparent" />
        <div
          className={`flex w-max gap-3 ${reduce ? "" : "animate-[weave-marquee_36s_linear_infinite]"}`}
        >
          {row.map((d, i) => (
            <span
              key={`${d}-${i}`}
              className="whitespace-nowrap rounded-xl border border-line bg-surface px-5 py-2.5 text-sm text-muted"
            >
              {d}
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}
