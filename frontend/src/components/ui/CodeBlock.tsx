"use client";

import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "@phosphor-icons/react";
import { cn } from "@/lib/cn";

type CopyState = "idle" | "copied" | "failed";

/**
 * Monospace panel with copy. `navigator.clipboard` is unavailable in some
 * browsers and insecure contexts, so the failure path is visible, not silent.
 */
export function CodeBlock({
  code,
  name,
  className,
  maxHeight = "22rem",
}: {
  code: string;
  name?: string;
  className?: string;
  maxHeight?: string;
}) {
  const [state, setState] = useState<CopyState>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setState("copied");
    } catch {
      setState("failed");
    }
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), 2200);
  }

  return (
    <div
      className={cn(
        "overflow-hidden rounded-2xl border border-line bg-[#0a0c0f]",
        className
      )}
    >
      <div className="flex items-center justify-between gap-3 border-b border-line/80 px-3 py-2">
        <span className="truncate font-mono text-[11px] uppercase tracking-[0.12em] text-faint">
          {name ?? "Example"}
        </span>
        <button
          type="button"
          onClick={() => void copy()}
          aria-label={`Copy ${name ?? "code"}`}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-2 py-1 text-[11px] text-faint transition-colors hover:bg-surface-2 hover:text-foreground"
        >
          {state === "copied" ? (
            <Check size={12} weight="bold" />
          ) : (
            <Copy size={12} />
          )}
          {state === "copied" ? "Copied" : state === "failed" ? "Copy failed" : "Copy"}
        </button>
      </div>
      <pre
        tabIndex={0}
        aria-label={name ? `${name} code` : "Example code"}
        className="weave-scroll overflow-auto p-4 font-mono text-[12px] leading-relaxed text-muted focus-visible:outline-offset-[-2px]"
        style={{ maxHeight }}
      >
        <code>{code}</code>
      </pre>
      <span className="sr-only" role="status">
        {state === "copied" ? "Copied to clipboard" : state === "failed" ? "Copy failed" : ""}
      </span>
    </div>
  );
}
