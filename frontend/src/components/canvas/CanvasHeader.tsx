"use client";

import { useGraphStore } from "@/lib/store";
import type { ViewType } from "@/lib/graph-types";

const VIEW_LABELS: Record<ViewType, string> = {
  default: "Default",
  topic: "Topic",
};

/** View tabs. Only functional views are shown; Topic lands in a later step. */
const VIEW_ORDER: ViewType[] = ["default"];

/** Compact overlay at the top of the canvas: brand mark + view switcher. */
export function CanvasHeader() {
  const viewConfig = useGraphStore((s) => s.viewConfig);
  const setViewConfig = useGraphStore((s) => s.setViewConfig);

  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-center justify-between border-b border-line bg-surface/80 px-4 py-2 backdrop-blur-md">
      <span className="text-sm font-semibold tracking-tight text-foreground">
        Weave
      </span>
      <div className="pointer-events-auto flex items-center gap-0.5 rounded-lg border border-line bg-background/80 p-0.5">
        {VIEW_ORDER.map((v) => (
          <button
            key={v}
            onClick={() => setViewConfig({ type: v })}
            className={[
              "rounded-md px-3 py-1 text-xs font-medium transition-colors",
              viewConfig.type === v
                ? "bg-accent text-accent-ink"
                : "text-muted hover:text-foreground",
            ].join(" ")}
          >
            {VIEW_LABELS[v]}
          </button>
        ))}
      </div>
    </div>
  );
}
