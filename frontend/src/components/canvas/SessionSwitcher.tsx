"use client";

import { useEffect, useRef, useState } from "react";
import {
  ArrowCounterClockwise,
  CaretDown,
  PencilSimple,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";
import {
  createSession,
  deleteSession,
  renameSession,
  resetActiveSession,
  switchSession,
} from "@/lib/persistence";

/** Dropdown to switch between named auto-saved project sessions and manage
 * them (create / rename / delete / reset). Persistence lives in
 * lib/persistence.ts; this is pure UI over it. */
export function SessionSwitcher() {
  const sessions = useGraphStore((s) => s.sessions);
  const activeSessionId = useGraphStore((s) => s.activeSessionId);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const active =
    sessions.find((s) => s.id === activeSessionId) ?? sessions[0];
  const sorted = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const handleNew = () => {
    createSession();
    setOpen(false);
  };

  const handleRename = (id: string, current: string) => {
    const name = window.prompt("Rename session", current);
    if (name !== null) renameSession(id, name);
  };

  const handleDelete = (id: string, name: string) => {
    if (!window.confirm(`Delete "${name}"? This cannot be undone.`)) return;
    deleteSession(id);
  };

  const handleReset = () => {
    if (!window.confirm("Clear this session's graph? The session stays.")) {
      return;
    }
    resetActiveSession();
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="pointer-events-auto relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Switch session"
        aria-haspopup="menu"
        aria-expanded={open}
        className="flex max-w-44 items-center gap-1.5 rounded-lg border border-line bg-background/80 px-2.5 py-1 text-xs font-medium text-muted transition-colors hover:text-foreground"
      >
        <span className="truncate">{active?.name ?? "Untitled"}</span>
        <CaretDown size={10} weight="bold" className="shrink-0" />
      </button>

      {open && (
        <div
          role="menu"
          className="absolute right-0 top-full z-30 mt-2 w-60 overflow-hidden rounded-xl border border-line bg-surface shadow-[0_16px_60px_rgba(0,0,0,0.5)]"
        >
          <div className="max-h-72 overflow-y-auto p-1">
            {sorted.length === 0 && (
              <p className="px-2 py-3 text-xs text-faint">No sessions yet.</p>
            )}
            {sorted.map((s) => {
              const isActive = s.id === activeSessionId;
              return (
                <div
                  key={s.id}
                  role="menuitem"
                  onClick={() => switchSession(s.id)}
                  className={[
                    "group flex cursor-pointer items-center gap-1 rounded-lg px-2 py-1.5 text-xs transition-colors",
                    isActive
                      ? "bg-accent/15 text-accent"
                      : "text-muted hover:bg-surface-2 hover:text-foreground",
                  ].join(" ")}
                >
                  <span className="min-w-0 flex-1 truncate">{s.name}</span>
                  <span className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRename(s.id, s.name);
                      }}
                      aria-label={`Rename ${s.name}`}
                      className="grid size-5 place-items-center rounded text-faint hover:text-foreground"
                    >
                      <PencilSimple size={11} />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(s.id, s.name);
                      }}
                      aria-label={`Delete ${s.name}`}
                      className="grid size-5 place-items-center rounded text-faint hover:text-red-400"
                    >
                      <Trash size={11} />
                    </button>
                  </span>
                </div>
              );
            })}
          </div>

          <div className="flex items-center gap-1 border-t border-line p-1">
            <button
              onClick={handleNew}
              className="flex flex-1 items-center justify-center gap-1 rounded-lg px-2 py-1.5 text-xs font-medium text-muted transition-colors hover:bg-surface-2 hover:text-foreground"
            >
              <Plus size={12} weight="bold" />
              New session
            </button>
            <button
              onClick={handleReset}
              aria-label="Reset session"
              title="Clear this session's graph"
              className="grid size-7 place-items-center rounded-lg text-faint transition-colors hover:bg-surface-2 hover:text-foreground"
            >
              <ArrowCounterClockwise size={13} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
