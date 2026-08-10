import Link from "next/link";

export function CtaFooter() {
  return (
    <>
      <section className="border-t border-line/60">
        <div className="mx-auto max-w-6xl px-6 py-28 text-center md:py-36">
          <h2 className="mx-auto max-w-xl text-3xl font-semibold tracking-tighter md:text-5xl">
            Stop organizing. Start understanding.
          </h2>
          <p className="mx-auto mt-5 max-w-md text-base leading-relaxed text-muted">
            Your next note is one sentence away from becoming a map.
          </p>
          <Link
            href="/app"
            className="mt-9 inline-block rounded-xl bg-accent px-7 py-3.5 text-sm font-medium text-accent-ink transition-transform active:scale-[0.97]"
          >
            Open the canvas
          </Link>
        </div>
      </section>

      <footer className="border-t border-line/60">
        <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-3 px-6 py-8 text-sm text-faint md:flex-row">
          <span>Weave</span>
          <span>Type it. See it. Know it.</span>
        </div>
      </footer>
    </>
  );
}
