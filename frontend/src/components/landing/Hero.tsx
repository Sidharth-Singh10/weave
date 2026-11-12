"use client";

import { motion, useReducedMotion } from "motion/react";
import { ArrowRight } from "@phosphor-icons/react";
import { canvasEntryHref } from "@/lib/demo-mode";
import { ButtonLink } from "@/components/ui/Button";
import { MiniGraphDemo } from "./MiniGraphDemo";

export function Hero() {
  const reduce = useReducedMotion();

  const fade = (delay: number) =>
    reduce
      ? {}
      : {
          initial: { opacity: 0, y: 18 },
          animate: { opacity: 1, y: 0 },
          transition: {
            duration: 0.7,
            delay,
            ease: [0.16, 1, 0.3, 1] as const,
          },
        };

  return (
    <section className="relative overflow-hidden">
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div className="weave-grid absolute inset-0 opacity-[0.16] [mask-image:radial-gradient(ellipse_at_62%_38%,black,transparent_74%)]" />
        <div className="absolute -right-40 -top-32 size-[560px] rounded-full bg-accent/[0.07] blur-[130px]" />
        <div className="weave-grain absolute inset-0 opacity-[0.04]" />
      </div>

      <div className="relative mx-auto grid w-full max-w-6xl grid-cols-1 items-center gap-12 px-6 pb-16 pt-14 sm:pt-16 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)] lg:gap-14 lg:pb-24 lg:pt-20">
        <div className="min-w-0">
          <motion.p
            {...fade(0)}
            className="font-mono text-[11px] uppercase tracking-[0.22em] text-accent"
          >
            Knowledge graphs, typed not drawn
          </motion.p>

          <motion.h1
            {...fade(0.08)}
            className="mt-5 max-w-[11ch] text-balance font-display text-[clamp(2.6rem,6vw,4.4rem)] font-semibold leading-[1.02] tracking-[-0.045em]"
          >
            Notes in. A living graph out.
          </motion.h1>

          <motion.p
            {...fade(0.16)}
            className="mt-5 max-w-md text-pretty text-[15px] leading-relaxed text-muted sm:text-base"
          >
            Weave extracts concepts and relationships as you write, and keeps
            the sentence behind every edge.
          </motion.p>

          <motion.div
            {...fade(0.24)}
            className="mt-8 flex flex-wrap items-center gap-x-6 gap-y-3"
          >
            <ButtonLink href={canvasEntryHref} size="lg">
              Open the canvas
            </ButtonLink>
            <a
              href="#how"
              className="inline-flex items-center gap-1.5 text-sm text-muted underline-offset-4 transition-colors hover:text-foreground hover:underline"
            >
              See how it works
              <ArrowRight size={14} />
            </a>
          </motion.div>
        </div>

        <motion.div
          {...(reduce
            ? {}
            : {
                initial: { opacity: 0, y: 24 },
                animate: { opacity: 1, y: 0 },
                transition: {
                  duration: 0.8,
                  delay: 0.18,
                  ease: [0.16, 1, 0.3, 1] as const,
                },
              })}
          className="relative min-w-0"
        >
          <div
            aria-hidden
            className="absolute -inset-px rounded-2xl bg-gradient-to-b from-accent/20 via-line/40 to-transparent opacity-70"
          />
          <div className="relative h-[462px] w-full min-w-0 overflow-hidden rounded-2xl border border-line bg-surface shadow-panel sm:h-[500px]">
            <MiniGraphDemo />
          </div>
        </motion.div>
      </div>
    </section>
  );
}
