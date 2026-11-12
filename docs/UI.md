# Weave UI

The public site and the product shell share one small design system.

## Tokens

Defined in `frontend/src/app/globals.css`:

| Token | Value | Use |
|---|---|---|
| `--background` | `#08090a` | Page base |
| `--surface` | `#0e1013` | Panels |
| `--surface-2` | `#15181c` | Controls, inset rows |
| `--surface-3` | `#1b1f24` | Hover fills |
| `--line` / `--line-bright` | `#24282e` / `#363c45` | Hairlines, hover borders |
| `--foreground` | `#f2f4f5` | Primary text |
| `--muted` | `#a8b0ba` | Body and secondary text |
| `--faint` | `#77818e` | Labels, metadata, captions |
| `--accent` / `--accent-bright` | `#34d399` / `#4ee6ad` | One UI accent |
| `--accent-ink` | `#042a1f` | Text on the accent |
| `--warning` / `--danger` | `#f5b14c` / `#f87171` | Semantic only |

Type: `Bricolage Grotesque` for display, `Geist` for UI and body, `Geist Mono`
for telemetry and code. Loaded through `next/font`.

Shape: panels `rounded-2xl` (16px), controls `rounded-xl` (12px), tags
`rounded-full`. Shadows use `shadow-panel` for framed surfaces and
`shadow-float` for overlays.

## Landing structure

`frontend/src/app/page.tsx` composes:

1. `Nav` - sticky, one line at desktop, disclosure at mobile.
2. `Hero` - live extractor, proof sentence, primary CTA.
3. `CapabilityBar` - five numbers from the repository.
4. `HowItWorks` - write, watch, refine.
5. `ExtractionSection` - deterministic versus bounded LLM.
6. `CanvasSection` - four semantic zoom levels and an evidence inspector.
7. `RetrievalSection` - ranked, explainable results.
8. `MemorySection` - the 18-tool MCP contract and claim lifecycle.
9. `ExportSection` - real JSON, Obsidian, GraphML, and Cypher output.
10. `PlatformSection` - the nine-stage ingest trace, policy, errors, architecture.
11. `UseCases` - six concrete scenarios and one domain marquee.
12. `CapabilityIndex` - 58 filterable capabilities.
13. `CtaFooter` - demo-aware close and project links.

Copy and numeric claims live in `frontend/src/lib/landing-content.ts`, which is
pure and covered by `src/lib/landing-content.test.ts`. The export demo graph is
in `frontend/src/lib/landing-demo-graph.ts`.

## Interaction rules

- `SegmentedControl` uses pressed buttons, not tabs, so every option stays
  keyboard reachable.
- All interactive demos are client islands with reduced-motion fallbacks.
- The hero graph seeds itself and reframes after each addition.
- The canvas graph defaults to `overview` on small screens, where `detail`
  would render unreadable text.
- Real generated exports are shown, not transcribed examples.
- No stock imagery or fake product screenshots. The product is the imagery.

## Product shell

The same tokens and `Logo` are used on `/login`, `/demo`, `/app`, and `/admin`.
Behavior is unchanged: routes, auth, store, persistence, and API calls were not
modified. `AdminNav` adds an active state and a mobile horizontal variant.

## Verification

```bash
cd frontend
npm run lint
npm test
npm run build
```

Browser checks cover 1440x900, 1024x768, 390x844, and 320px, plus keyboard
navigation, the accessibility tree, zero console errors, and the interactive
sections. `e2e/landing.spec.ts` runs the landing checks in Playwright; the
authenticated canvas and admin surfaces still use the existing auth flow.
