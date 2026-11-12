"use client";

import { useState, useSyncExternalStore } from "react";
import { commonGroundEnabled, getCommonGroundDocId } from "@/lib/common-ground";
import { CommonGroundPanel } from "./CommonGroundPanel";

const subscribe = () => () => {};

/** Shows the adapter's document id; an agent targets this id with graph envelopes. */
export function CommonGroundBadge() {
  const [open, setOpen] = useState(false);
  const docId = useSyncExternalStore(subscribe, getCommonGroundDocId, () => "");
  if (!commonGroundEnabled || !docId) return null;
  return (
    <>
      <button
        type="button"
        title={`Common Ground document: ${docId}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="pointer-events-auto rounded-full border border-line bg-background/60 px-2 py-0.5 font-mono text-[10px] text-faint transition-colors hover:border-line-bright hover:text-foreground"
      >
        CG {docId.slice(-8)}
      </button>
      {open && <CommonGroundPanel onClose={() => setOpen(false)} />}
    </>
  );
}
