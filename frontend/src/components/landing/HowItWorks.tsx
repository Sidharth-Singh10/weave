"use client";

import { GitBranch, PencilLine, Sliders } from "@phosphor-icons/react";
import { HOW_IT_WORKS } from "@/lib/landing-content";
import { Reveal } from "@/components/ui/Reveal";
import { SectionHeading } from "@/components/ui/SectionHeading";

const ICONS = [PencilLine, GitBranch, Sliders];

export function HowItWorks() {
  return (
    <section id="how" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <SectionHeading
          title="Thinking is the job. Drawing is ours."
          lead="Three habits replace an entire toolbar of manual mapping. Everything else is optional."
        />

        <div className="mt-14 grid grid-cols-1 gap-px border-t border-line bg-line md:grid-cols-3">
          {HOW_IT_WORKS.map((beat, index) => {
            const Icon = ICONS[index] ?? PencilLine;
            return (
              <Reveal key={beat.step} delay={index * 0.08} className="bg-background">
                <div className="h-full px-0 py-6 md:px-6 md:py-7">
                  <div className="flex items-center justify-between gap-4">
                    <span className="font-mono text-[11px] tracking-[0.18em] text-faint">
                      {beat.step}
                    </span>
                    <Icon size={18} className="text-accent" />
                  </div>
                  <h3 className="mt-6 text-lg font-medium tracking-tight text-foreground">
                    {beat.title}
                  </h3>
                  <p className="mt-2 max-w-sm text-sm leading-relaxed text-muted">
                    {beat.body}
                  </p>
                </div>
              </Reveal>
            );
          })}
        </div>
      </div>
    </section>
  );
}
