"use client";

import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { motion } from "motion/react";
import { useGraphStore, type WeaveFlowNode } from "@/lib/store";

export function WeaveNode({ id, data, selected }: NodeProps<WeaveFlowNode>) {
  const renameNode = useGraphStore((s) => s.renameNode);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(data.label);

  const commit = () => {
    const next = draft.trim();
    if (next && next !== data.label) renameNode(id, next);
    setEditing(false);
  };

  if (data.ghost) {
    return (
      <motion.div
        initial={{ opacity: 0, scale: 0.7 }}
        animate={{ opacity: [0.35, 0.6, 0.35], scale: 1 }}
        transition={{
          opacity: { duration: 1.8, repeat: Infinity, ease: "easeInOut" },
          scale: { type: "spring", stiffness: 300, damping: 24 },
        }}
        className="rounded-xl border border-line/50 bg-surface/60 px-4 py-2.5 shadow-[0_8px_30px_rgba(0,0,0,0.25)]"
      >
        <Handle type="target" position={Position.Top} className="!opacity-0" />
        <Handle type="source" position={Position.Bottom} className="!opacity-0" />
        <div className="flex items-center gap-2">
          <span className="inline-block size-2 rounded-full bg-accent/60" />
          <span className="text-sm text-muted/70">Weaving...</span>
        </div>
      </motion.div>
    );
  }

  return (
    <motion.div
      initial={data.fresh ? { opacity: 0, scale: 0.6 } : false}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ type: "spring", stiffness: 320, damping: 24 }}
      onDoubleClick={() => {
        setDraft(data.label);
        setEditing(true);
      }}
      className={[
        "group relative rounded-xl border bg-surface px-4 py-2.5 shadow-[0_8px_30px_rgba(0,0,0,0.35)]",
        selected ? "border-accent" : "border-line hover:border-faint",
        data.fresh ? "ring-2 ring-accent/60" : "",
      ].join(" ")}
    >
      <Handle type="target" position={Position.Top} className="!opacity-0" />
      <Handle type="source" position={Position.Bottom} className="!opacity-0" />

      {editing ? (
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setEditing(false);
          }}
          className="w-36 rounded-md border border-accent bg-background px-2 py-0.5 text-sm text-foreground outline-none"
        />
      ) : (
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-foreground">
            {data.label}
          </span>
          {data.kind && (
            <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
              {data.kind}
            </span>
          )}
        </div>
      )}
    </motion.div>
  );
}
