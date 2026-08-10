"use client";

import { useState } from "react";
import { motion } from "motion/react";
import { ArrowUp, WarningCircle } from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";

export function InputDock() {
  const [value, setValue] = useState("");
  const status = useGraphStore((s) => s.status);
  const error = useGraphStore((s) => s.error);
  const submit = useGraphStore((s) => s.submit);
  const clearError = useGraphStore((s) => s.clearError);

  const thinking = status === "thinking";

  const send = () => {
    const text = value.trim();
    if (!text || thinking) return;
    setValue("");
    void submit(text);
  };

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-6 z-10 flex flex-col items-center gap-2 px-4">
      {status === "error" && error && (
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className="pointer-events-auto flex items-center gap-2 rounded-xl border border-line bg-surface px-4 py-2 text-sm text-muted"
        >
          <WarningCircle size={16} className="text-accent" />
          <span>Could not read that note. {error}</span>
          <button
            onClick={clearError}
            className="ml-1 text-foreground underline-offset-2 hover:underline"
          >
            Dismiss
          </button>
        </motion.div>
      )}

      <motion.div
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ type: "spring", stiffness: 200, damping: 24, delay: 0.15 }}
        className="pointer-events-auto relative w-full max-w-2xl"
      >
        <div className="flex items-center gap-2 rounded-2xl border border-line bg-surface/90 p-2 shadow-[0_16px_60px_rgba(0,0,0,0.5)] backdrop-blur-md">
          <input
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") send();
            }}
            placeholder="Type a thought. Weave adds it to the graph."
            aria-label="Add a note to the graph"
            disabled={thinking}
            className="min-w-0 flex-1 bg-transparent px-3 py-2 text-base text-foreground placeholder:text-faint focus:outline-none disabled:opacity-60"
          />
          <button
            onClick={send}
            disabled={thinking || !value.trim()}
            aria-label="Add note"
            className="grid size-10 shrink-0 place-items-center rounded-xl bg-accent text-accent-ink transition-transform active:scale-[0.96] disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-faint"
          >
            <ArrowUp size={18} weight="bold" />
          </button>
        </div>

        {thinking && (
          <motion.div
            aria-hidden
            className="absolute inset-x-6 -bottom-px h-px origin-left bg-accent"
            initial={{ scaleX: 0 }}
            animate={{ scaleX: [0, 0.7, 0.9] }}
            transition={{ duration: 2.4, ease: "easeInOut" }}
          />
        )}
      </motion.div>
    </div>
  );
}
