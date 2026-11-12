"use client";

import { useEffect, useRef, useState } from "react";
import {
  ArrowSquareOut,
  DownloadSimple,
  Graph,
  BookOpenText,
  LockKey,
} from "@phosphor-icons/react";
import {
  exportCypher,
  exportGraphML,
  exportJson,
  exportObsidian,
} from "@/lib/export";
import {
  EXPORT_DEMO_DATE,
  EXPORT_DEMO_INPUT,
  obsidianShowcaseFile,
} from "@/lib/landing-demo-graph";
import {
  EXPORT_FORMATS,
  type ExportFormatId,
} from "@/lib/landing-content";
import { downloadBytes, downloadText } from "@/lib/download";
import { zipSync } from "@/lib/zip";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { CodeBlock } from "@/components/ui/CodeBlock";
import { SegmentedControl } from "@/components/ui/SegmentedControl";
import { SectionHeading } from "@/components/ui/SectionHeading";

const OBSIDIAN_FILES = exportObsidian(EXPORT_DEMO_INPUT, EXPORT_DEMO_DATE);
const OBSIDIAN_SAMPLE = obsidianShowcaseFile(OBSIDIAN_FILES);

const SNIPPETS: Record<ExportFormatId, { code: string; name: string; file: string }> = {
  json: {
    code: exportJson(EXPORT_DEMO_INPUT, EXPORT_DEMO_DATE),
    name: "weave-ada-lovelace.json",
    file: "weave-ada-lovelace.json",
  },
  obsidian: {
    code: OBSIDIAN_SAMPLE.content,
    name: OBSIDIAN_SAMPLE.path,
    file: "weave-ada-lovelace-obsidian.zip",
  },
  graphml: {
    code: exportGraphML(EXPORT_DEMO_INPUT),
    name: "weave-ada-lovelace.graphml",
    file: "weave-ada-lovelace.graphml",
  },
  cypher: {
    code: exportCypher(EXPORT_DEMO_INPUT),
    name: "weave-ada-lovelace.cypher",
    file: "weave-ada-lovelace.cypher",
  },
};

const TABS = EXPORT_FORMATS.map((format) => ({
  value: format.id,
  label: format.label,
}));

export function ExportSection() {
  const [format, setFormat] = useState<ExportFormatId>("json");
  const [note, setNote] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const option = EXPORT_FORMATS.find((item) => item.id === format) ?? EXPORT_FORMATS[0];
  const snippet = SNIPPETS[format];

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  function download() {
    if (format === "obsidian") {
      downloadBytes(snippet.file, zipSync(OBSIDIAN_FILES), "application/zip");
    } else {
      downloadText(snippet.file, snippet.code, option.mime);
    }
    setNote(`Downloaded ${snippet.file}`);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setNote(null), 2600);
  }

  return (
    <section id="exports" className="border-t border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-20 md:py-28">
        <SectionHeading
          title="Take the graph with you."
          lead="The graph lives in your browser, so the export is the durable copy. Four formats are generated client-side, and the schema is versioned independently of in-browser migrations."
        />

        <div className="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
          <div className="min-w-0 rounded-2xl border border-line bg-surface p-4 shadow-panel">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <SegmentedControl
                label="Export format"
                value={format}
                onChange={setFormat}
                options={TABS}
              />
              <button
                type="button"
                onClick={download}
                className="inline-flex items-center gap-1.5 rounded-xl border border-line bg-background px-3 py-1.5 text-xs text-muted transition-colors hover:border-line-bright hover:text-foreground"
              >
                <DownloadSimple size={13} />
                Download sample
              </button>
            </div>
            <CodeBlock
              name={snippet.name}
              code={snippet.code}
              maxHeight="25rem"
            />
            <div className="mt-3 flex items-center justify-between gap-2 text-[11px] text-faint">
              <span>
                {option.extension} · {EXPORT_DEMO_INPUT.nodes.length} concepts ·{" "}
                {EXPORT_DEMO_INPUT.edges.length} relationships
              </span>
              <span role="status" aria-live="polite" className="text-accent">
                {note ?? ""}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <div className="rounded-2xl border border-line bg-surface p-5">
              <div className="flex items-center justify-between gap-2">
                <Badge tone="accent">client-side</Badge>
                <Graph size={18} className="text-faint" />
              </div>
              <h3 className="mt-4 font-display text-xl font-semibold tracking-[-0.03em]">
                {option.label}
              </h3>
              <p className="mt-2 text-sm leading-relaxed text-muted">
                {option.useFor}
              </p>
              <div className="mt-4 border-t border-line pt-3">
                <p className="text-[11px] leading-relaxed text-faint">
                  Nothing is uploaded. The JSON snapshot carries a stable
                  schema_version and inlined provenance; the Obsidian vault ships
                  an index and one linked note per concept.
                </p>
              </div>
            </div>

            <div className="rounded-2xl border border-line bg-surface p-5">
              <div className="flex items-center gap-2">
                <BookOpenText size={18} className="text-accent" />
                <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-faint">
                  common ground
                </p>
              </div>
              <h3 className="mt-3 font-display text-xl font-semibold tracking-[-0.03em]">
                Agent edits join the same journal.
              </h3>
              <p className="mt-2 text-sm leading-relaxed text-muted">
                The client-owned graph connects to a journaled protocol. Human
                edits and agent operations appear side by side, destructive
                operations wait for one-shot approval, and undo is a capability.
              </p>
              <div className="mt-4 flex flex-wrap gap-1.5">
                {["cap_graph", "cap_human", "cap_undo"].map((capability) => (
                  <span
                    key={capability}
                    className="inline-flex items-center gap-1 rounded-md border border-line bg-background px-1.5 py-0.5 font-mono text-[10px] text-muted"
                  >
                    <LockKey size={10} className="text-accent" />
                    {capability}
                  </span>
                ))}
              </div>
              <a
                href="#platform"
                className="mt-4 inline-flex items-center gap-1.5 text-xs text-muted transition-colors hover:text-foreground"
              >
                See the platform path
                <ArrowSquareOut size={12} />
              </a>
            </div>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-2 gap-px border border-line bg-line sm:grid-cols-4">
          {EXPORT_FORMATS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setFormat(item.id)}
              aria-pressed={item.id === format}
              className={`bg-background px-4 py-3 text-left transition-colors hover:bg-surface ${
                item.id === format ? "text-foreground" : "text-muted"
              }`}
            >
              <span className="font-mono text-[10px] text-faint">
                {item.extension}
              </span>
              <span className="mt-1 block text-xs font-medium">{item.label}</span>
            </button>
          ))}
        </div>

        <div className="mt-6 flex flex-wrap items-center gap-3">
          <Button
            variant="secondary"
            size="sm"
            onClick={download}
            className="gap-1.5"
          >
            <DownloadSimple size={14} />
            Download {option.label}
          </Button>
          <p className="text-xs text-faint">
            Generated in the browser. This sample uses the same export functions
            as the canvas.
          </p>
        </div>
      </div>
    </section>
  );
}
