"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { motion } from "motion/react";
import { ArrowUp } from "@phosphor-icons/react";
import { ingestNote } from "@/lib/api";
import { DEMO_SCRIPT, type DemoStep } from "@/lib/demo";

interface DemoNodeData extends Record<string, unknown> {
  label: string;
  fresh: boolean;
}

type DemoNode = Node<DemoNodeData>;

/** Stable id from the label, so the state updater stays pure under StrictMode. */
const nodeId = (label: string) => `mini:${label.trim().toLowerCase()}`;

function MiniNode({ data }: NodeProps<DemoNode>) {
  return (
    <motion.div
      initial={data.fresh ? { opacity: 0, scale: 0.6 } : false}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ type: "spring", stiffness: 320, damping: 24 }}
      className={`flex items-center gap-1.5 rounded-lg border bg-surface px-2.5 py-1.5 text-[11px] font-medium ${
        data.fresh
          ? "border-accent text-foreground"
          : "border-line text-foreground/85"
      }`}
    >
      <Handle type="target" position={Position.Top} className="!opacity-0" />
      <span
        aria-hidden
        className={`size-1.5 shrink-0 rounded-full ${
          data.fresh ? "bg-accent" : "bg-line-bright"
        }`}
      />
      {data.label}
      <Handle type="source" position={Position.Bottom} className="!opacity-0" />
    </motion.div>
  );
}

const demoNodeTypes = { mini: MiniNode };

function DemoFlow() {
  const [nodes, setNodes] = useState<DemoNode[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [liveFailed, setLiveFailed] = useState(false);
  const clearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { fitView } = useReactFlow();

  const clearFresh = useCallback(() => {
    if (clearTimer.current) clearTimeout(clearTimer.current);
    clearTimer.current = setTimeout(() => {
      setNodes((prev) =>
        prev.map((node) =>
          node.data.fresh
            ? { ...node, data: { ...node.data, fresh: false } }
            : node
        )
      );
    }, 1400);
  }, []);

  useEffect(() => {
    return () => {
      if (clearTimer.current) clearTimeout(clearTimer.current);
    };
  }, []);

  const addScripted = useCallback(
    (step: DemoStep) => {
      setNodes((prev) => {
        const next = [...prev];
        for (const node of step.nodes) {
          const id = nodeId(node.label);
          if (next.some((item) => item.id === id)) continue;
          next.push({
            id,
            type: "mini",
            position: { x: node.x, y: node.y },
            data: { label: node.label, fresh: true },
          });
        }
        return next;
      });
      setEdges((prev) => {
        const next = [...prev];
        for (const edge of step.edges) {
          const id = `${nodeId(edge.from)}->${nodeId(edge.to)}:${edge.relation}`;
          if (next.some((item) => item.id === id)) continue;
          next.push({
            id,
            source: nodeId(edge.from),
            target: nodeId(edge.to),
            label: edge.relation,
          });
        }
        return next;
      });
      clearFresh();
    },
    [clearFresh]
  );

  // Reframe whenever the graph grows. Without this, a phone viewport clips the
  // nodes that were added after the initial fitView.
  useEffect(() => {
    if (nodes.length === 0) return;
    const timer = setTimeout(() => {
      void fitView({ padding: 0.3, maxZoom: 1, duration: 320 });
    }, 60);
    return () => clearTimeout(timer);
  }, [nodes.length, edges.length, fitView]);

  // Populate the hero graph on first paint so the demo reads as a product,
  // not an empty box. Re-adding a sentence is idempotent.
  useEffect(() => {
    const first = setTimeout(() => addScripted(DEMO_SCRIPT[0]), 0);
    const second = setTimeout(() => addScripted(DEMO_SCRIPT[1]), 600);
    const third = setTimeout(() => addScripted(DEMO_SCRIPT[2]), 1350);
    return () => {
      clearTimeout(first);
      clearTimeout(second);
      clearTimeout(third);
    };
  }, [addScripted]);

  const sendLive = useCallback(async () => {
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    setBusy(true);
    try {
      const delta = await ingestNote({
        text,
        nodes: nodes.map((node) => ({ label: node.data.label, kind: "" })),
        edges: [],
      });

      const occupied: [number, number][] = nodes.map((node) => [
        node.position.x,
        node.position.y,
      ]);
      const freeSpot = (): { x: number; y: number } => {
        for (let i = 0; i < 12; i++) {
          const angle = Math.random() * Math.PI * 2;
          const radius = 130 + Math.random() * 120;
          const point = {
            x: Math.round(radius * Math.cos(angle)),
            y: Math.round(radius * Math.sin(angle)),
          };
          if (
            occupied.every(
              ([x, y]) => Math.hypot(x - point.x, y - point.y) > 90
            )
          ) {
            occupied.push([point.x, point.y]);
            return point;
          }
        }
        const fallback = { x: occupied.length * 90 - 180, y: 160 };
        occupied.push([fallback.x, fallback.y]);
        return fallback;
      };

      setNodes((prev) => {
        const next = [...prev];
        for (const node of delta.nodes) {
          const id = nodeId(node.label);
          if (next.some((item) => item.id === id)) continue;
          next.push({
            id,
            type: "mini",
            position: freeSpot(),
            data: { label: node.label, fresh: true },
          });
        }
        return next;
      });
      setEdges((prev) => {
        const next = [...prev];
        for (const edge of delta.edges) {
          const id = `${nodeId(edge.source_label)}->${nodeId(
            edge.target_label
          )}:${edge.relation}`;
          if (next.some((item) => item.id === id)) continue;
          next.push({
            id,
            source: nodeId(edge.source_label),
            target: nodeId(edge.target_label),
            label: edge.relation,
          });
        }
        return next;
      });
      clearFresh();
    } catch {
      setLiveFailed(true);
      setInput(text);
      setTimeout(() => setLiveFailed(false), 4000);
    } finally {
      setBusy(false);
    }
  }, [input, busy, nodes, clearFresh]);

  const status = liveFailed
    ? "Live extractor offline. Start the API and retry."
    : busy
      ? "Weaving the note into the graph."
      : "";

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex items-center justify-between gap-2 border-b border-line px-3 py-2">
        <div className="flex items-center gap-2">
          <span className="size-1.5 rounded-full bg-accent" />
          <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted">
            Live extractor
          </span>
        </div>
        <span className="hidden font-mono text-[10px] text-faint sm:block">
          POST /api/graph/ingest
        </span>
      </div>

      <div
        role="img"
        aria-label="Live knowledge graph built from the example sentences"
        className="relative h-[242px] shrink-0 sm:h-[278px]"
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={demoNodeTypes}
          nodesConnectable={false}
          nodesDraggable
          elementsSelectable={false}
          zoomOnScroll={false}
          panOnDrag
          preventScrolling={false}
          minZoom={0.4}
          maxZoom={1.4}
          fitView
          fitViewOptions={{ padding: 0.35, maxZoom: 1 }}
          proOptions={{ hideAttribution: true }}
          defaultEdgeOptions={{
            labelStyle: { fill: "#a8b0ba", fontSize: 9 },
            labelBgStyle: { fill: "#0e1013", fillOpacity: 0.92 },
            style: { stroke: "#363c45", strokeWidth: 1.4 },
          }}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={24}
            size={1}
            color="#24282e"
          />
        </ReactFlow>

        {nodes.length === 0 && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center px-6 text-center">
            <p className="max-w-[22ch] text-xs leading-relaxed text-faint">
              Start with an example, or type a sentence.
            </p>
          </div>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col border-t border-line p-3">
        <div className="flex flex-wrap gap-1.5">
          {DEMO_SCRIPT.map((step) => (
            <button
              key={step.sentence}
              type="button"
              onClick={() => addScripted(step)}
              aria-label={`Add the example: ${step.sentence}`}
              className="max-w-full truncate rounded-lg border border-line bg-surface-2 px-2.5 py-1 text-left text-[10.5px] text-muted transition-colors hover:border-line-bright hover:text-foreground active:scale-[0.98]"
            >
              {step.sentence}
            </button>
          ))}
        </div>

        <form
          className="mt-2.5"
          onSubmit={(event) => {
            event.preventDefault();
            void sendLive();
          }}
        >
          <div className="flex items-center justify-between font-mono text-[10px] uppercase tracking-[0.12em] text-faint">
            <label htmlFor="hero-note">Your note</label>
            <span aria-hidden>Enter to weave</span>
          </div>
          <div className="mt-1.5 flex items-center gap-2 rounded-xl border border-line bg-background p-1.5 focus-within:border-line-bright">
            <input
              id="hero-note"
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder="Katherine Johnson worked at NASA..."
              className="min-w-0 flex-1 bg-transparent px-2 py-1.5 text-sm text-foreground outline-none placeholder:text-faint"
            />
            <button
              type="submit"
              disabled={busy || !input.trim()}
              aria-label="Run live extraction"
              className="grid size-8 shrink-0 place-items-center rounded-lg bg-accent text-accent-ink transition-transform active:scale-[0.96] disabled:bg-surface-2 disabled:text-faint"
            >
              <ArrowUp size={15} weight="bold" />
            </button>
          </div>
        </form>

        <p
          role="status"
          aria-live="polite"
          className="mt-2 min-h-4 font-mono text-[10px] leading-4 tracking-wide text-faint"
        >
          {status}
        </p>
      </div>
    </div>
  );
}

export function MiniGraphDemo() {
  return (
    <ReactFlowProvider>
      <DemoFlow />
    </ReactFlowProvider>
  );
}
