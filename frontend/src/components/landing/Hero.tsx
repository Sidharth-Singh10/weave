"use client";

import Link from "next/link";
import { motion, useReducedMotion } from "motion/react";
import { MiniGraphDemo } from "./MiniGraphDemo";

export function Hero() {
  const reduce = useReducedMotion();

  const fade = (delay: number) =>
    reduce
      ? {}
      : {
          initial: { opacity: 0, y: 20 },
          animate: { opacity: 1, y: 0 },
          transition: { duration: 0.7, delay, ease: [0.16, 1, 0.3, 1] as const },
        };

  return (
    <section className="mx-auto grid min-h-[calc(100dvh-4rem)] w-full max-w-6xl grid-cols-1 items-center gap-10 px-6 pb-16 pt-16 lg:grid-cols-2 lg:gap-14 lg:pt-10">
      {/* Copy */}
      <div>
        <motion.p
          {...fade(0)}
          className="font-mono text-xs uppercase tracking-[0.2em] text-accent"
        >
          Knowledge graphs, typed not drawn
        </motion.p>

        <motion.h1
          {...fade(0.08)}
          className="mt-5 text-4xl font-semibold leading-[1.05] tracking-tighter md:text-6xl"
        >
          Your notes, woven into a graph
        </motion.h1>

        <motion.p
          {...fade(0.16)}
          className="mt-5 max-w-md text-base leading-relaxed text-muted"
        >
          Type in plain language. Weave pulls out the concepts and connections,
          and draws the map for you.
        </motion.p>

        <motion.div {...fade(0.24)} className="mt-8 flex items-center gap-4">
          <Link
            href="/app"
            className="rounded-xl bg-accent px-6 py-3 text-sm font-medium text-accent-ink transition-transform active:scale-[0.97]"
          >
            Open the canvas
          </Link>
          <a
            href="#how"
            className="text-sm text-muted underline-offset-4 transition-colors hover:text-foreground hover:underline"
          >
            See how it works
          </a>
        </motion.div>
      </div>

      {/* Live demo */}
      <motion.div
        {...(reduce
          ? {}
          : {
              initial: { opacity: 0, y: 28 },
              animate: { opacity: 1, y: 0 },
              transition: { duration: 0.8, delay: 0.2, ease: [0.16, 1, 0.3, 1] as const },
            })}
        className="h-[420px] overflow-hidden rounded-2xl border border-line bg-surface shadow-[0_24px_80px_rgba(0,0,0,0.45)] md:h-[480px]"
      >
        <MiniGraphDemo />
      </motion.div>
    </section>
  );
}
