"use client";

import { useState } from "react";
import {
  adminRunPipeline,
  ApiError,
  type PipelineStage,
  type PipelineTrace,
} from "@/lib/api";
import type { IngestRequest } from "@/lib/graph-types";

const STAGE_LABELS: Record<string, string> = {
  auth: "Authenticate",
  permission: "Check permission",
  policy: "Resolve policy",
  rate_limit: "Rate limit",
  quota: "Token quota",
  concurrency: "Concurrency slot",
  extract: "LLM extraction",
  usage: "Record usage",
  response: "Respond",
};

export default function PipelinePage() {
  const [text, setText] = useState("Hogwarts has four houses.");
  const [graphJson, setGraphJson] = useState(`{ "nodes": [], "edges": [] }`);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [trace, setTrace] = useState<PipelineTrace | null>(null);

  async function run() {
    setLoading(true);
    setError(null);
    setTrace(null);
    let req: IngestRequest;
    try {
      const parsed = JSON.parse(graphJson || "{}");
      req = {
        text,
        nodes: Array.isArray(parsed.nodes) ? parsed.nodes : [],
        edges: Array.isArray(parsed.edges) ? parsed.edges : [],
      };
    } catch {
      setError("Existing graph must be valid JSON, e.g. { \"nodes\": [], \"edges\": [] }");
      setLoading(false);
      return;
    }
    try {
      setTrace(await adminRunPipeline(req));
    } catch (e: unknown) {
      if (e instanceof ApiError) {
        setError(`${e.message} (${e.code})`);
      } else {
        setError(e instanceof Error ? e.message : "Failed to run the pipeline.");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mx-auto max-w-6xl">
      <div className="mb-4">
        <h1 className="text-lg font-semibold tracking-tight">Pipeline</h1>
        <p className="mt-1 text-sm text-muted">
          Observe the <span className="font-mono text-xs">POST /api/graph/ingest</span> flow,
          observe-only: nothing is blocked or mutated. Rate-limit / quota / concurrency checks run
          and report what they would do.
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <div className="space-y-3">
          <div className="rounded-xl border border-line bg-surface p-4">
            <label className="mb-1.5 block text-xs font-medium text-faint" htmlFor="pipeline-note">
              New note
            </label>
            <textarea
              id="pipeline-note"
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={4}
              className="w-full resize-y rounded-lg border border-line bg-background p-3 text-sm text-foreground outline-none focus:border-accent"
            />
          </div>

          <div className="rounded-xl border border-line bg-surface p-4">
            <label className="mb-1.5 block text-xs font-medium text-faint" htmlFor="pipeline-graph">
              Existing graph sent to the LLM
            </label>
            <textarea
              id="pipeline-graph"
              value={graphJson}
              onChange={(e) => setGraphJson(e.target.value)}
              rows={6}
              spellCheck={false}
              className="w-full resize-y rounded-lg border border-line bg-background p-3 font-mono text-xs text-foreground outline-none focus:border-accent"
            />
            <p className="mt-1.5 text-xs text-faint">
              JSON shape: <span className="font-mono">{"{ nodes: {label, kind}[], edges: {source_label, target_label, relation}[] }"}</span>
            </p>
          </div>

          <button
            onClick={run}
            disabled={loading}
            className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accent-ink transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {loading ? "Running…" : "Run pipeline"}
          </button>

          {error && (
            <div className="rounded-lg border border-line bg-surface px-4 py-3 text-sm text-muted">
              {error}
            </div>
          )}
        </div>

        <div className="min-w-0">
          {loading ? (
            <p className="text-sm text-faint">Tracing…</p>
          ) : trace ? (
            <TraceView trace={trace} />
          ) : (
            <p className="text-sm text-faint">Run the pipeline to see each stage end to end.</p>
          )}
        </div>
      </div>
    </div>
  );
}

function TraceView({ trace }: { trace: PipelineTrace }) {
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted">
        <span className="rounded-md border border-line bg-surface px-2 py-1 font-mono">
          {trace.endpoint}
        </span>
        <span className="rounded-md border border-line bg-surface px-2 py-1">
          {trace.total_ms} ms total
        </span>
        <span className="rounded-md border border-line bg-surface px-2 py-1">
          LLM mode: {trace.llm_mode}
        </span>
      </div>

      <ol className="space-y-2">
        {trace.stages.map((s, i) => (
          <StageRow key={s.stage} stage={s} index={i} />
        ))}
      </ol>
    </div>
  );
}

function StageRow({ stage, index }: { stage: PipelineStage; index: number }) {
  const [open, setOpen] = useState(index >= 6);
  const label = STAGE_LABELS[stage.stage] ?? stage.stage;
  const badge = stageBadge(stage);

  return (
    <li className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-3 rounded-xl border border-line bg-surface px-4 py-3 text-left transition-colors hover:border-zinc-700"
      >
        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-line text-xs text-faint">
          {index + 1}
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-sm font-medium">{label}</span>
          <span className="block font-mono text-xs text-faint">{stage.stage}</span>
        </span>
        <span className="rounded-md border border-line bg-surface-2 px-2 py-0.5 text-xs text-muted">
          {stage.duration_ms} ms
        </span>
        <span className={badge.className}>{badge.text}</span>
        <span className="text-xs text-faint">{open ? "▾" : "▸"}</span>
      </button>

      {open && (
        <div className="mt-1 space-y-3 rounded-xl border border-line bg-background p-4">
          {stage.stage === "extract" && <ExtractDetail stage={stage} />}
          <div>
            <h4 className="mb-1 text-xs font-medium text-faint">Detail</h4>
            <JsonBlock value={stage.detail} />
          </div>
        </div>
      )}
    </li>
  );
}

function stageBadge(stage: PipelineStage) {
  if (stage.status === "error") {
    return { text: "error", className: "rounded-md bg-red-500/15 px-2 py-0.5 text-xs font-medium text-red-400" };
  }
  const d = stage.detail as Record<string, unknown>;
  if (stage.stage === "extract" && stage.status === "mock") {
    return { text: "mock", className: "rounded-md bg-zinc-500/15 px-2 py-0.5 text-xs font-medium text-zinc-400" };
  }
  if (stage.stage === "rate_limit" && d.would_block) {
    return { text: "would block", className: "rounded-md bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-400" };
  }
  if (stage.stage === "quota" && d.exhausted) {
    return { text: "exhausted", className: "rounded-md bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-400" };
  }
  if (stage.stage === "concurrency" && d.acquired === false && d.configured === true) {
    return { text: "limited", className: "rounded-md bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-400" };
  }
  return { text: "ok", className: "rounded-md bg-emerald-500/15 px-2 py-0.5 text-xs font-medium text-emerald-400" };
}

function ExtractDetail({ stage }: { stage: PipelineStage }) {
  const d = stage.detail as Record<string, unknown>;
  return (
    <div className="space-y-3">
      <SubgraphSummary d={d} />
      <div>
        <h4 className="mb-1 text-xs font-medium text-faint">System prompt</h4>
        <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface p-3 font-mono text-xs text-muted">
          {String(d.system_prompt ?? "")}
        </pre>
      </div>
      <div>
        <h4 className="mb-1 text-xs font-medium text-faint">User prompt (new note + selected graph + rules)</h4>
        <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface p-3 font-mono text-xs text-muted">
          {String(d.user_prompt ?? "")}
        </pre>
      </div>
      {d.llm_raw_response != null && (
        <div>
          <h4 className="mb-1 text-xs font-medium text-faint">Raw LLM response</h4>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface p-3 font-mono text-xs text-muted">
            {String(d.llm_raw_response)}
          </pre>
        </div>
      )}
    </div>
  );
}

function SubgraphSummary({ d }: { d: Record<string, unknown> }) {
  const num = (v: unknown) => (typeof v === "number" ? v : 0);
  const anchors = Array.isArray(d.anchors) ? (d.anchors as string[]) : [];
  const rows = [
    { label: "anchors", value: anchors.length ? anchors.join(", ") : "(none)" },
    { label: "selected", value: `${num(d.subgraph_node_count)} nodes · ${num(d.subgraph_edge_count)} edges` },
    { label: "omitted", value: `${num(d.omitted_node_count)} nodes · ${num(d.omitted_edge_count)} edges` },
    { label: "est. tokens", value: String(num(d.estimated_tokens)) },
    { label: "hops", value: String(num(d.max_hops)) },
  ];
  return (
    <div>
      <h4 className="mb-1.5 text-xs font-medium text-faint">Subgraph selection (bounded context)</h4>
      <div className="space-y-1 rounded-lg border border-line bg-surface p-3">
        {rows.map((r) => (
          <div key={r.label} className="flex gap-2 text-xs">
            <span className="w-16 shrink-0 text-faint">{r.label}</span>
            <span className="min-w-0 break-words font-mono text-muted">{r.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-surface p-3 font-mono text-xs text-muted">
      {JSON.stringify(value, null, 2)}
    </pre>
  );
}