"use client";

import { useCallback, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
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

function MiniNode({ data }: NodeProps<DemoNode>) {
  return (
    <motion.div
      initial={data.fresh ? { opacity: 0, scale: 0.6 } : false}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ type: "spring", stiffness: 320, damping: 24 }}
      className={`rounded-lg border bg-surface px-3 py-1.5 text-xs font-medium text-foreground ${
        data.fresh ? "border-accent" : "border-line"
      }`}
    >
      <Handle type="target" position={Position.Top} className="!opacity-0" />
      <Handle type="source" position={Position.Bottom} className="!opacity-0" />
      {data.label}
    </motion.div>
  );
}

const demoNodeTypes = { mini: MiniNode };



let demoId = 0;
const nextDemoId = () => `d${++demoId}`;

function DemoFlow() {
  const [nodes, setNodes] = useState<DemoNode[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [liveFailed, setLiveFailed] = useState(false);
  const idMap = useRef(new Map<string, string>());

  const addScripted = useCallback((step: DemoStep) => {
    setNodes((prev) => {
      const next = [...prev];
      for (const n of step.nodes) {
        const key = n.label.toLowerCase();
        if (idMap.current.has(key)) continue;
        const id = nextDemoId();
        idMap.current.set(key, id);
        next.push({
          id,
          type: "mini",
          position: { x: n.x, y: n.y },
          data: { label: n.label, fresh: true },
        });
      }
      return next;
    });
    setEdges((prev) => {
      const next = [...prev];
      for (const e of step.edges) {
        const src = idMap.current.get(e.from.toLowerCase());
        const tgt = idMap.current.get(e.to.toLowerCase());
        if (!src || !tgt) continue;
        const id = `${src}->${tgt}:${e.relation}`;
        if (next.some((x) => x.id === id)) continue;
        next.push({ id, source: src, target: tgt, label: e.relation });
      }
      return next;
    });
    setTimeout(() => {
      setNodes((prev) =>
        prev.map((n) => (n.data.fresh ? { ...n, data: { ...n.data, fresh: false } } : n))
      );
    }, 1400);
  }, []);

  const sendLive = useCallback(async () => {
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    setBusy(true);
    try {
      const delta = await ingestNote({
        text,
        nodes: nodes.map((n) => ({ label: n.data.label, kind: "" })),
        edges: [],
      });

      const occupied: [number, number][] = nodes.map((n) => [
        n.position.x,
        n.position.y,
      ]);
      const freeSpot = (): { x: number; y: number } => {
        for (let i = 0; i < 12; i++) {
          const angle = Math.random() * Math.PI * 2;
          const r = 130 + Math.random() * 120;
          const p = { x: Math.round(r * Math.cos(angle)), y: Math.round(r * Math.sin(angle)) };
          if (
            occupied.every(([ox, oy]) => Math.hypot(ox - p.x, oy - p.y) > 90)
          ) {
            occupied.push([p.x, p.y]);
            return p;
          }
        }
        const fallback = { x: occupied.length * 90 - 180, y: 160 };
        occupied.push([fallback.x, fallback.y]);
        return fallback;
      };

      setNodes((prev) => {
        const next = [...prev];
        for (const n of delta.nodes) {
          const key = n.label.toLowerCase();
          if (idMap.current.has(key)) continue;
          const id = nextDemoId();
          idMap.current.set(key, id);
          next.push({ id, type: "mini", position: freeSpot(), data: { label: n.label, fresh: true } });
        }
        return next;
      });
      setEdges((prev) => {
        const next = [...prev];
        for (const e of delta.edges) {
          const src = idMap.current.get(e.source_label.toLowerCase());
          const tgt = idMap.current.get(e.target_label.toLowerCase());
          if (!src || !tgt) continue;
          const id = `${src}->${tgt}:${e.relation}`;
          if (next.some((x) => x.id === id)) continue;
          next.push({ id, source: src, target: tgt, label: e.relation });
        }
        return next;
      });
      setTimeout(() => {
        setNodes((prev) =>
          prev.map((n) => (n.data.fresh ? { ...n, data: { ...n.data, fresh: false } } : n))
        );
      }, 1400);
    } catch {
      setLiveFailed(true);
      setInput(text);
      setTimeout(() => setLiveFailed(false), 4000);
    } finally {
      setBusy(false);
    }
  }, [input, busy, nodes]);

  return (
    <div className="flex h-full flex-col">
      <div className="relative h-[270px] md:h-[330px]">
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
            labelStyle: { fill: "#a1a1aa", fontSize: 10 },
            labelBgStyle: { fill: "#101013", fillOpacity: 0.9 },
            style: { stroke: "#3f3f46", strokeWidth: 1.5 },
          }}
        >
          <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="#27272a" />
        </ReactFlow>

        {nodes.length === 0 && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center">
            <p className="text-sm text-faint">Pick a sentence below, or type your own.</p>
          </div>
        )}
      </div>

      <div className="border-t border-line p-3">
        <div className="mb-3 flex flex-wrap gap-2">
          {DEMO_SCRIPT.map((step) => (
            <button
              key={step.sentence}
              onClick={() => addScripted(step)}
              className="rounded-lg border border-line bg-surface-2 px-3 py-1.5 text-left text-xs text-muted transition-colors hover:border-faint hover:text-foreground active:scale-[0.98]"
            >
              {step.sentence}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2 rounded-xl border border-line bg-background p-1.5">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void sendLive();
            }}
            placeholder="Or type a sentence..."
            aria-label="Try a sentence against the live extractor"
            className="min-w-0 flex-1 bg-transparent px-2.5 py-1.5 text-sm text-foreground placeholder:text-faint focus:outline-none"
          />
          <button
            onClick={() => void sendLive()}
            disabled={busy || !input.trim()}
            aria-label="Run live extraction"
            className="grid size-8 shrink-0 place-items-center rounded-lg bg-accent text-accent-ink transition-transform active:scale-[0.96] disabled:bg-surface-2 disabled:text-faint"
          >
            <ArrowUp size={15} weight="bold" />
          </button>
        </div>
        <p className="mt-2 h-4 font-mono text-[10px] tracking-wide text-faint">
          {liveFailed
            ? "Live extractor offline. Start the backend and retry."
            : busy
              ? "Weaving..."
              : ""}
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
