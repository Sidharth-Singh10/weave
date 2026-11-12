"use client";

import { useMemo, useState } from "react";
import { GithubLogo } from "@phosphor-icons/react";
import {
  CAPABILITY_CATEGORIES,
  CAPABILITY_GROUPS,
  type CapabilityCategory,
} from "@/lib/landing-content";
import { SegmentedControl } from "@/components/ui/SegmentedControl";
import { SectionHeading } from "@/components/ui/SectionHeading";

type Filter = CapabilityCategory | "All";

const FILTER_OPTIONS: { value: Filter; label: string }[] = [
  { value: "All", label: "All" },
  ...CAPABILITY_CATEGORIES.map((category) => ({
    value: category,
    label: category,
  })),
];

const TOTAL = CAPABILITY_GROUPS.reduce(
  (count, group) => count + group.items.length,
  0
);

export function CapabilityIndex() {
  const [filter, setFilter] = useState<Filter>("All");

  const groups = useMemo(
    () =>
      filter === "All"
        ? CAPABILITY_GROUPS
        : CAPABILITY_GROUPS.filter((group) => group.category === filter),
    [filter]
  );

  const visible = groups.reduce(
    (count, group) => count + group.items.length,
    0
  );

  return (
    <section id="capabilities" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <SectionHeading
            title="Every capability, in one place."
            lead="The features above, plus the platform work that does not fit in a headline. Filter the index or read it straight through."
          />
          <div className="shrink-0 text-left lg:text-right">
            <div className="font-display text-4xl font-semibold tracking-[-0.05em]">
              {TOTAL}
            </div>
            <p className="mt-1 font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              shipped capabilities
            </p>
          </div>
        </div>

        <div className="mt-8 flex flex-wrap items-center justify-between gap-3">
          <SegmentedControl
            label="Filter capabilities"
            value={filter}
            onChange={setFilter}
            options={FILTER_OPTIONS}
            size="sm"
          />
          <p
            role="status"
            aria-live="polite"
            className="font-mono text-[11px] text-faint"
          >
            showing {visible} of {TOTAL}
          </p>
        </div>

        <div className="mt-10 grid grid-cols-1 gap-x-10 gap-y-10 md:grid-cols-2 lg:grid-cols-3">
          {groups.map((group) => (
            <div key={group.category}>
              <div className="flex items-baseline justify-between gap-2">
                <h3 className="font-mono text-[10px] uppercase tracking-[0.16em] text-accent">
                  {group.category}
                </h3>
                <span className="font-mono text-[10px] text-faint">
                  {group.items.length}
                </span>
              </div>
              <ul className="mt-3 divide-y divide-line border-t border-line">
                {group.items.map((item) => (
                  <li key={item.title} className="py-2.5">
                    <h4 className="text-[13px] font-medium text-foreground">
                      {item.title}
                    </h4>
                    <p className="mt-0.5 text-[11px] leading-relaxed text-faint">
                      {item.detail}
                    </p>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-10 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-6">
          <p className="max-w-xl text-xs leading-relaxed text-faint">
            Every item is implemented in this repository. The README documents the
            routes, schemas, tests, and deployment topology behind them.
          </p>
          <a
            href="https://github.com/Sidharth-Singh10/weave#features"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1.5 text-xs text-muted transition-colors hover:text-foreground"
          >
            <GithubLogo size={14} />
            Read the README
          </a>
        </div>
      </div>
    </section>
  );
}
