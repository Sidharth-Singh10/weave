import type { HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

/** A bordered surface. Used only where the boundary carries hierarchy. */
export function Panel({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-2xl border border-line bg-surface", className)}
      {...props}
    />
  );
}
