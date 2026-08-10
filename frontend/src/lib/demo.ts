export interface DemoStep {
  sentence: string;
  nodes: { label: string; x: number; y: number }[];
  edges: { from: string; to: string; relation: string }[];
}

export const DEMO_SCRIPT: DemoStep[] = [
  {
    sentence: "Harry Potter",
    nodes: [{ label: "Harry Potter", x: 0, y: 0 }],
    edges: [],
  },
  {
    sentence: "Harry's best friends are Ron and Hermione.",
    nodes: [
      { label: "Ron", x: -150, y: 90 },
      { label: "Hermione", x: 150, y: 90 },
    ],
    edges: [
      { from: "Harry Potter", to: "Ron", relation: "friend of" },
      { from: "Harry Potter", to: "Hermione", relation: "friend of" },
    ],
  },
  {
    sentence: "Harry studies at Hogwarts.",
    nodes: [{ label: "Hogwarts", x: 0, y: -120 }],
    edges: [{ from: "Harry Potter", to: "Hogwarts", relation: "studies at" }],
  },
];
