"use client";

import { motion, useReducedMotion } from "motion/react";
import {
  Sparkle,
  Infinity as InfinityIcon,
  Copy,
  HandPalm,
} from "@phosphor-icons/react";
import { LiveGraphAnimation } from "./LiveGraphAnimation";

const cells = [
  {
    icon: Sparkle,
    title: "Extraction on every note",
    body: "Characters, places, topics, and the verbs between them become nodes and edges the moment you hit enter.",
    className: "md:col-span-2 bg-surface",
    ink: false,
    hasAnimation: true,
  },
  {
    icon: InfinityIcon,
    title: "One canvas, forever",
    body: "No new documents. The map grows where your thinking grows.",
    className:
      "bg-gradient-to-br from-accent to-accent-dim text-accent-ink border-transparent",
    ink: true,
  },
  {
    icon: Copy,
    title: "No duplicates",
    body: "Mention Ron twice, there is still one Ron.",
    className:
      "bg-surface-2 [background-image:radial-gradient(#27272a_1px,transparent_1px)] [background-size:18px_18px]",
    ink: false,
  },
  {
    icon: HandPalm,
    title: "You stay in charge",
    body: "AI suggests, you decide. Drag, rename, merge, or delete anything it gets wrong.",
    className: "md:col-span-2 bg-surface",
    ink: false,
  },
];

export function Features() {
  const reduce = useReducedMotion();

  return (
    <section className="border-t border-line/60">
      <div className="mx-auto max-w-6xl px-6 py-24 md:py-32">
        <h2 className="max-w-lg text-3xl font-semibold tracking-tighter md:text-4xl">
          Built for the way knowledge actually grows
        </h2>

        <div className="mt-14 grid grid-cols-1 gap-4 md:grid-cols-3">
          {cells.map((cell, i) => (
            <motion.div
              key={cell.title}
              initial={reduce ? false : { opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, amount: 0.3 }}
              transition={{
                duration: 0.55,
                delay: i * 0.08,
                ease: [0.16, 1, 0.3, 1],
              }}
              className={`rounded-2xl border border-line p-7 ${cell.className}`}
            >
              {'hasAnimation' in cell && cell.hasAnimation ? (
                <div className="flex items-start gap-6">
                  <div className="min-w-0 flex-1">
                    <cell.icon
                      size={22}
                      className="text-accent"
                    />
                    <h3 className="mt-4 text-lg font-medium tracking-tight text-foreground">
                      {cell.title}
                    </h3>
                    <p className="mt-2 max-w-md text-sm leading-relaxed text-muted">
                      {cell.body}
                    </p>
                  </div>
                  <div className="hidden h-28 w-48 shrink-0 md:block">
                    <LiveGraphAnimation />
                  </div>
                </div>
              ) : (
                <>
                  <cell.icon
                    size={22}
                    className={cell.ink ? "text-accent-ink" : "text-accent"}
                  />
                  <h3
                    className={`mt-4 text-lg font-medium tracking-tight ${
                      cell.ink ? "text-accent-ink" : "text-foreground"
                    }`}
                  >
                    {cell.title}
                  </h3>
                  <p
                    className={`mt-2 max-w-md text-sm leading-relaxed ${
                      cell.ink ? "text-accent-ink/80" : "text-muted"
                    }`}
                  >
                    {cell.body}
                  </p>
                </>
              )}
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
