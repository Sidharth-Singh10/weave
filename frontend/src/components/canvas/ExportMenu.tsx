"use client";

import { useEffect, useRef, useState } from "react";
import {
  BookOpenText,
  CaretDown,
  Database,
  DownloadSimple,
  FileCode,
  Graph,
} from "@phosphor-icons/react";
import { useGraphStore } from "@/lib/store";
import {
  exportCypher,
  exportGraphML,
  exportJson,
  exportObsidian,
  type ExportFormat,
  type ExportInput,
} from "@/lib/export";
import { zipSync } from "@/lib/zip";
import { downloadBytes, downloadText } from "@/lib/download";

const FORMATS: {
  id: ExportFormat;
  label: string;
  hint: string;
  icon: typeof FileCode;
}[] = [
  {
    id: "json",
    label: "JSON snapshot",
    hint: "Everything, with provenance",
    icon: FileCode,
  },
  {
    id: "obsidian",
    label: "Obsidian vault",
    hint: "Notes linked with [[wikilinks]]",
    icon: BookOpenText,
  },
  {
    id: "graphml",
    label: "GraphML",
    hint: "Gephi, yEd, NetworkX",
    icon: Graph,
  },
  {
    id: "cypher",
    label: "Cypher",
    hint: "Neo4j MERGE statements",
    icon: Database,
  },
];

function fileSlug(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
  return slug || "graph";
}

/**
 * Download the current graph. The knowledge graph otherwise exists only in
 * this browser's IndexedDB, so this menu is the escape hatch.
 */
export function ExportMenu() {
  const knowledgeNodes = useGraphStore((s) => s.knowledgeNodes);
  const knowledgeEdges = useGraphStore((s) => s.knowledgeEdges);
  const positions = useGraphStore((s) => s.positions);
  const nodeSources = useGraphStore((s) => s.nodeSources);
  const edgeSources = useGraphStore((s) => s.edgeSources);
  const communityLabels = useGraphStore((s) => s.communityLabels);
  const sessionName = useGraphStore(
    (s) =>
      s.sessions.find((session) => session.id === s.activeSessionId)?.name ??
      null
  );

  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const empty = knowledgeNodes.length === 0;

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function handle(format: ExportFormat) {
    setOpen(false);

    const input: ExportInput = {
      nodes: knowledgeNodes,
      edges: knowledgeEdges,
      positions,
      nodeSources,
      edgeSources,
      communityLabels,
      sessionName,
    };

    const date = new Date().toISOString().slice(0, 10);
    const base = `weave-${fileSlug(sessionName ?? "graph")}-${date}`;

    switch (format) {
      case "json":
        downloadText(`${base}.json`, exportJson(input), "application/json");
        break;
      case "obsidian":
        downloadBytes(
          `${base}-obsidian.zip`,
          zipSync(exportObsidian(input)),
          "application/zip"
        );
        break;
      case "graphml":
        downloadText(`${base}.graphml`, exportGraphML(input), "application/graphml+xml");
        break;
      case "cypher":
        downloadText(`${base}.cypher`, exportCypher(input), "text/plain");
        break;
    }
  }

  return (
    <div ref={rootRef} className="pointer-events-auto relative">
      <button
        onClick={() => setOpen((value) => !value)}
        disabled={empty}
        aria-label="Export graph"
        aria-haspopup="menu"
        aria-expanded={open}
        title={empty ? "Add a note before exporting" : "Export this graph"}
        className="flex items-center gap-1.5 rounded-xl border border-line bg-background/80 px-2.5 py-1 text-xs font-medium text-muted transition-colors hover:border-line-bright hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
      >
        <DownloadSimple size={12} />
        Export
        <CaretDown size={10} weight="bold" className="shrink-0" />
      </button>

      {open && (
        <div
          role="menu"
          aria-label="Export format"
          className="absolute right-0 top-full z-30 mt-2 w-64 overflow-hidden rounded-2xl border border-line bg-surface shadow-float"
        >
          <div className="p-1">
            {FORMATS.map((format) => (
              <button
                key={format.id}
                role="menuitem"
                onClick={() => handle(format.id)}
                className="flex w-full items-start gap-2 rounded-xl px-2 py-1.5 text-left transition-colors hover:bg-surface-2"
              >
                <format.icon
                  size={14}
                  className="mt-0.5 shrink-0 text-faint"
                />
                <span className="min-w-0">
                  <span className="block text-xs font-medium text-foreground">
                    {format.label}
                  </span>
                  <span className="block text-[11px] text-faint">
                    {format.hint}
                  </span>
                </span>
              </button>
            ))}
          </div>
          <p className="border-t border-line px-3 py-2 text-[11px] text-faint">
            {knowledgeNodes.length} concepts · {knowledgeEdges.length} relationships
          </p>
        </div>
      )}
    </div>
  );
}
