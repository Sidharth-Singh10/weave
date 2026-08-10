"use client";

import { useCallback } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  SelectionMode,
  useReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useGraphStore } from "@/lib/store";
import type { WeaveFlowNode } from "@/lib/graph-types";
import { WeaveNode } from "./WeaveNode";
import { InputDock } from "./InputDock";

const nodeTypes = { weave: WeaveNode };

function Flow() {
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const onNodesChange = useGraphStore((s) => s.onNodesChange);
  const onEdgesChange = useGraphStore((s) => s.onEdgesChange);
  const onConnect = useGraphStore((s) => s.onConnect);
  const { fitView } = useReactFlow();

  const onInit = useCallback(() => {
    fitView({ padding: 0.3, duration: 0 });
  }, [fitView]);

  return (
    <div className="relative h-[100dvh] w-full overflow-hidden bg-background">
      <ReactFlow<WeaveFlowNode>
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onInit={onInit}
        panOnScroll
        selectionOnDrag
        selectionMode={SelectionMode.Partial}
        deleteKeyCode={["Backspace", "Delete"]}
        minZoom={0.2}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
        defaultEdgeOptions={{
          labelStyle: { fill: "#a1a1aa", fontSize: 11 },
          labelBgStyle: { fill: "#101013", fillOpacity: 0.9 },
          style: { stroke: "#3f3f46", strokeWidth: 1.5 },
        }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={28}
          size={1}
          color="#27272a"
        />
        <Controls
          position="bottom-right"
          showInteractive={false}
          className="!border-line !bg-surface [&_button]:!border-line [&_button]:!bg-surface [&_button]:!fill-muted"
        />
      </ReactFlow>

      {nodes.length === 0 && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center">
          <div className="text-center">
            <p className="text-lg text-muted">Your canvas is empty.</p>
            <p className="mt-1 text-sm text-faint">
              Type one idea below. Weave starts drawing.
            </p>
          </div>
        </div>
      )}

      <InputDock />
    </div>
  );
}

export function CanvasApp() {
  return (
    <ReactFlowProvider>
      <Flow />
    </ReactFlowProvider>
  );
}
