import type { Metadata } from "next";
import { CanvasApp } from "@/components/canvas/CanvasApp";

export const metadata: Metadata = {
  title: "Weave - canvas",
};

export default function AppPage() {
  return <CanvasApp />;
}
