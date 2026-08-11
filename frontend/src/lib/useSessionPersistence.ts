"use client";

import { useEffect } from "react";
import { initPersistence } from "./persistence";

/** Mount once inside the canvas. Wires the graph store to the debounced
 * session saver and hydrates the active session on load. */
export function useSessionPersistence() {
  useEffect(() => {
    return initPersistence();
  }, []);
}
