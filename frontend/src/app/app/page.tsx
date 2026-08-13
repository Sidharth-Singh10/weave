import type { Metadata } from "next";
import { CanvasApp } from "@/components/canvas/CanvasApp";
import { RequireAuth } from "@/components/auth/RequireAuth";

export const metadata: Metadata = {
  title: "Weave - canvas",
};

export default function AppPage() {
  return (
    <RequireAuth>
      <CanvasApp />
    </RequireAuth>
  );
}
