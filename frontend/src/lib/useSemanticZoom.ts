"use client";

import { useCallback, useRef } from "react";
import { useReactFlow } from "@xyflow/react";
import type { SemanticZoomLevel } from "./graph-types";
import { useGraphStore } from "./store";

/** Map a ReactFlow zoom scale to a semantic information-density level. */
export function zoomToLevel(zoom: number): SemanticZoomLevel {
  if (zoom < 0.5) return "overview";
  if (zoom < 0.9) return "category";
  if (zoom < 1.5) return "entity";
  return "detail";
}

/**
 * Keep the store's viewConfig.semanticZoom in sync with the current camera
 * zoom. Only updates when the semantic level actually changes, so zooming
 * within one level doesn't spam re-renders.
 */
export function useSemanticZoom(): () => void {
  const { getZoom } = useReactFlow();
  const setViewConfig = useGraphStore((s) => s.setViewConfig);
  const lastLevel = useRef<SemanticZoomLevel | null>(null);

  return useCallback(() => {
    const level = zoomToLevel(getZoom());
    if (level !== lastLevel.current) {
      lastLevel.current = level;
      setViewConfig({ semanticZoom: level });
    }
  }, [getZoom, setViewConfig]);
}
