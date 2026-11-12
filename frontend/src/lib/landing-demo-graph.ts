import type { ExportInput } from "./export.ts";
import type { KnowledgeEdge, KnowledgeNode, SourceRef } from "./graph-types.ts";

/**
 * A fixed, deterministic graph used by the export showcase. The date and ids
 * never change, so server and client produce identical code samples and the
 * downloaded files are reproducible.
 */
export const EXPORT_DEMO_DATE = new Date("2026-01-12T09:14:00.000Z");

const CREATED_AT = EXPORT_DEMO_DATE.getTime();

const ADA_NOTE =
  "Ada Lovelace wrote the first algorithm intended for the Analytical Engine.";
const BABBAGE_NOTE =
  "Charles Babbage designed the Analytical Engine and corresponded with Ada.";
const NOTES_NOTE =
  "Ada Lovelace published her notes on the Analytical Engine in 1843.";

const NODES: KnowledgeNode[] = [
  { id: "n_ada", label: "Ada Lovelace", kind: "person" },
  { id: "n_engine", label: "Analytical Engine", kind: "machine" },
  { id: "n_babbage", label: "Charles Babbage", kind: "person" },
  { id: "n_notes", label: "Notes", kind: "document" },
];

const EDGES: KnowledgeEdge[] = [
  {
    id: "e_programmed",
    source: "n_ada",
    target: "n_engine",
    relation: "programmed",
    confidence: "EXTRACTED",
    evidence: ADA_NOTE,
  },
  {
    id: "e_corresponded",
    source: "n_ada",
    target: "n_babbage",
    relation: "corresponded with",
    confidence: "EXTRACTED",
    evidence: BABBAGE_NOTE,
  },
  {
    id: "e_designed",
    source: "n_babbage",
    target: "n_engine",
    relation: "designed",
    confidence: "EXTRACTED",
    evidence: BABBAGE_NOTE,
  },
  {
    id: "e_wrote",
    source: "n_ada",
    target: "n_notes",
    relation: "wrote",
    confidence: "INFERRED",
    evidence: NOTES_NOTE,
  },
];

const NODE_SOURCES: Record<string, SourceRef> = {
  n_ada: { note: ADA_NOTE, createdAt: CREATED_AT },
  n_engine: { note: BABBAGE_NOTE, createdAt: CREATED_AT },
  n_babbage: { note: BABBAGE_NOTE, createdAt: CREATED_AT },
  n_notes: { note: NOTES_NOTE, createdAt: CREATED_AT },
};

const EDGE_SOURCES: Record<string, SourceRef> = {
  e_programmed: { note: ADA_NOTE, createdAt: CREATED_AT },
  e_corresponded: { note: BABBAGE_NOTE, createdAt: CREATED_AT },
  e_designed: { note: BABBAGE_NOTE, createdAt: CREATED_AT },
  e_wrote: { note: NOTES_NOTE, createdAt: CREATED_AT },
};

export const EXPORT_DEMO_INPUT: ExportInput = {
  nodes: NODES,
  edges: EDGES,
  positions: {
    n_ada: { x: 0, y: 0 },
    n_engine: { x: 280, y: -90 },
    n_babbage: { x: 280, y: 110 },
    n_notes: { x: -240, y: -100 },
  },
  nodeSources: NODE_SOURCES,
  edgeSources: EDGE_SOURCES,
  communityLabels: {
    "0": "Ada's circle",
    "1": "The engine",
  },
  sessionName: "Ada Lovelace",
};

/** The showable Obsidian file from the demo vault. */
export function obsidianShowcaseFile(
  files: { path: string; content: string }[]
): { path: string; content: string } {
  return (
    files.find((file) => file.path === "Nodes/Ada Lovelace.md") ?? files[0]
  );
}
