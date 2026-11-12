import { cn } from "@/lib/cn";

/**
 * Brand mark. Two strands crossing inside a rounded tile: the weave, reduced to
 * its simplest form. The wordmark is part of the logo, not an icon.
 */
export function Logo({
  className,
  wordmark = true,
  size = 26,
}: {
  className?: string;
  wordmark?: boolean;
  size?: number;
}) {
  return (
    <span className={cn("inline-flex items-center gap-2.5", className)}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        fill="none"
        aria-hidden="true"
        className="shrink-0"
      >
        <rect
          x="1"
          y="1"
          width="30"
          height="30"
          rx="9"
          fill="var(--surface-2)"
          stroke="var(--line)"
        />
        <path
          d="M7 11.5c4.5 0 4.5 9 9 9s4.5-9 9-9"
          stroke="var(--accent)"
          strokeWidth="1.9"
          strokeLinecap="round"
        />
        <path
          d="M7 20.5c4.5 0 4.5-9 9-9s4.5 9 9 9"
          stroke="var(--foreground)"
          strokeWidth="1.2"
          strokeLinecap="round"
          opacity="0.55"
        />
      </svg>
      {wordmark && (
        <span className="font-display text-[17px] font-semibold tracking-[-0.03em] text-foreground">
          Weave
        </span>
      )}
    </span>
  );
}
