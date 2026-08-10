"use client";

import { motion, useReducedMotion } from "motion/react";

const gNodes = [
  { cx: 40, cy: 50, delay: 0 },
  { cx: 120, cy: 25, delay: 0.25 },
  { cx: 110, cy: 85, delay: 0.45 },
  { cx: 190, cy: 55, delay: 0.65 },
  { cx: 195, cy: 100, delay: 0.8 },
];

const gEdges = [
  { from: 0, to: 1, delay: 0.3 },
  { from: 0, to: 2, delay: 0.5 },
  { from: 1, to: 3, delay: 0.7 },
  { from: 2, to: 4, delay: 0.85 },
  { from: 3, to: 4, delay: 0.95 },
];

export function LiveGraphAnimation() {
  const reduce = useReducedMotion();

  return (
    <svg
      viewBox="0 0 230 130"
      fill="none"
      className="h-full w-full"
      aria-hidden
    >
      {gEdges.map((e, i) => (
        <motion.line
          key={i}
          x1={gNodes[e.from].cx}
          y1={gNodes[e.from].cy}
          x2={gNodes[e.to].cx}
          y2={gNodes[e.to].cy}
          stroke="var(--line)"
          strokeWidth={1.5}
          initial={reduce ? false : { opacity: 0 }}
          whileInView={{ opacity: 0.6 }}
          viewport={{ once: true, amount: 0.5 }}
          transition={{ duration: 0.5, delay: e.delay }}
        />
      ))}
      {gNodes.map((n, i) => (
        <motion.circle
          key={i}
          cx={n.cx}
          cy={n.cy}
          r={5}
          fill="var(--surface)"
          stroke="var(--accent)"
          strokeWidth={1.5}
          style={{ transformOrigin: `${n.cx}px ${n.cy}px` }}
          initial={reduce ? false : { scale: 0, opacity: 0 }}
          whileInView={{ scale: 1, opacity: 1 }}
          viewport={{ once: true, amount: 0.5 }}
          transition={{
            type: "spring",
            stiffness: 300,
            damping: 20,
            delay: n.delay,
          }}
        />
      ))}
    </svg>
  );
}
