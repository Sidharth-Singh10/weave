import { Nav } from "@/components/landing/Nav";
import { Hero } from "@/components/landing/Hero";
import { CapabilityBar } from "@/components/landing/CapabilityBar";
import { HowItWorks } from "@/components/landing/HowItWorks";
import { ExtractionSection } from "@/components/landing/ExtractionSection";
import { CanvasSection } from "@/components/landing/CanvasSection";
import { RetrievalSection } from "@/components/landing/RetrievalSection";
import { MemorySection } from "@/components/landing/MemorySection";
import { ExportSection } from "@/components/landing/ExportSection";
import { PlatformSection } from "@/components/landing/PlatformSection";
import { UseCases } from "@/components/landing/UseCases";
import { CapabilityIndex } from "@/components/landing/CapabilityIndex";
import { CtaFooter } from "@/components/landing/CtaFooter";

export default function Landing() {
  return (
    <>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[60] focus:rounded-xl focus:bg-accent focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-accent-ink"
      >
        Skip to content
      </a>
      <Nav />
      <main id="main" className="flex-1">
        <Hero />
        <CapabilityBar />
        <HowItWorks />
        <ExtractionSection />
        <CanvasSection />
        <RetrievalSection />
        <MemorySection />
        <ExportSection />
        <PlatformSection />
        <UseCases />
        <CapabilityIndex />
      </main>
      <CtaFooter />
    </>
  );
}
