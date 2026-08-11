"use client";

import { create } from "zustand";

/** Transient UI status surfaced by the persistence layer: quota warnings,
 * the "restored session" toast, and the last successful save time. */
interface PersistenceStatusState {
  quotaExceeded: boolean;
  restoredSession: string | null;
  lastSavedAt: number | null;
  setQuotaExceeded: (value: boolean) => void;
  setRestoredSession: (name: string | null) => void;
  setLastSavedAt: (time: number | null) => void;
}

export const usePersistenceStatus = create<PersistenceStatusState>()((set) => ({
  quotaExceeded: false,
  restoredSession: null,
  lastSavedAt: null,
  setQuotaExceeded: (value) => set({ quotaExceeded: value }),
  setRestoredSession: (name) => set({ restoredSession: name }),
  setLastSavedAt: (time) => set({ lastSavedAt: time }),
}));
