import { Nav } from "@/components/landing/Nav";
import { Hero } from "@/components/landing/Hero";
import { HowItWorks } from "@/components/landing/HowItWorks";
import { Features } from "@/components/landing/Features";
import { UseCases } from "@/components/landing/UseCases";
import { CtaFooter } from "@/components/landing/CtaFooter";

export default function Landing() {
  return (
    <main>
      <Nav />
      <Hero />
      <HowItWorks />
      <Features />
      <UseCases />
      <CtaFooter />
    </main>
  );
}
