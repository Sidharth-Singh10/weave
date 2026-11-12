"use client";

import { useSyncExternalStore } from "react";

/**
 * Read a media query without a render-time layout read. The server snapshot is
 * false, so server components and hydration agree; the client snapshot then
 * reflects the real viewport.
 */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const media = window.matchMedia(query);
      media.addEventListener("change", onChange);
      return () => media.removeEventListener("change", onChange);
    },
    () => window.matchMedia(query).matches,
    () => false
  );
}
