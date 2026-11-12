import { SPEC_BAR } from "@/lib/landing-content";

/** The five numbers that explain the system before the feature sections. */
export function CapabilityBar() {
  return (
    <section aria-label="Weave at a glance" className="border-y border-line bg-surface/30">
      <div className="mx-auto grid max-w-6xl grid-cols-2 gap-px bg-line md:grid-cols-5">
        {SPEC_BAR.map((item, index) => (
          <div
            key={item.value}
            className={`bg-background px-5 py-5 md:px-4 ${
              index === SPEC_BAR.length - 1 ? "col-span-2 md:col-span-1" : ""
            }`}
          >
            <div className="font-display text-2xl font-semibold tracking-[-0.04em] text-foreground md:text-[1.65rem]">
              {item.value}
            </div>
            <p className="mt-1.5 text-pretty text-[11px] leading-relaxed text-faint">
              {item.label}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}
