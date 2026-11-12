import { cn } from "@/lib/cn";

/** Section title and lead. No kicker, no split header: the title owns the row. */
export function SectionHeading({
  title,
  lead,
  id,
  className,
}: {
  title: string;
  lead?: string;
  id?: string;
  className?: string;
}) {
  return (
    <div className={cn("max-w-3xl", className)}>
      <h2
        id={id}
        className="text-balance font-display text-3xl font-semibold leading-[1.05] tracking-[-0.04em] text-foreground sm:text-4xl md:text-[2.75rem]"
      >
        {title}
      </h2>
      {lead && (
        <p className="mt-4 max-w-2xl text-pretty text-[15px] leading-relaxed text-muted md:text-base">
          {lead}
        </p>
      )}
    </div>
  );
}
