"use client";

import { motion, useReducedMotion } from "motion/react";
import { PencilLine, GitBranch, Sliders } from "@phosphor-icons/react";

const beats = [
  {
    icon: PencilLine,
    title: "Write",
    body: "Drop a sentence into the input, exactly as you would say it. No nodes to place, no connectors to drag.",
  },
  {
    icon: GitBranch,
    title: "Watch",
    body: "Weave reads the note, finds the concepts and the relationships, and extends your graph in place.",
  },
  {
    icon: Sliders,
    title: "Refine",
    body: "Drag anything, rename anything, delete the links that are wrong. The graph is yours to correct.",
  },
];

export function HowItWorks() {
  const reduce = useReducedMotion();

  return (
    <section id="how" className="border-t border-line/60">
      <div className="mx-auto max-w-6xl px-6 py-24 md:py-32">
        <h2 className="max-w-md text-3xl font-semibold tracking-tighter md:text-4xl">
          Thinking is the job. Drawing is ours.
        </h2>
        <p className="mt-4 max-w-xl text-base leading-relaxed text-muted">
          Three habits replace an entire toolbar of manual mapping.
        </p>

        <div className="mt-16 grid grid-cols-1 gap-10 md:grid-cols-3 md:gap-8">
          {beats.map((beat, i) => (
            <motion.div
              key={beat.title}
              initial={reduce ? false : { opacity: 0, y: 24 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, amount: 0.4 }}
              transition={{
                duration: 0.6,
                delay: i * 0.12,
                ease: [0.16, 1, 0.3, 1],
              }}
              className={i === 1 ? "md:mt-10" : i === 2 ? "md:mt-20" : ""}
            >
              <div className="grid size-11 place-items-center rounded-xl border border-line bg-surface">
                <beat.icon size={20} className="text-accent" />
              </div>
              <h3 className="mt-5 text-lg font-medium tracking-tight">
                {beat.title}
              </h3>
              <p className="mt-2 text-sm leading-relaxed text-muted">
                {beat.body}
              </p>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
