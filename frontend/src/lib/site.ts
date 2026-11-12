/**
 * Public site constants. The deployment URL is overridable at build time so
 * Open Graph and canonical metadata are correct when the site moves.
 */
export const siteUrl = (
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://weave.mutref.tech"
).replace(/\/+$/, "");

export const siteName = "Weave";

export const siteTitle = "Weave - notes into a living knowledge graph";

export const siteDescription =
  "Type a note and Weave extracts concepts and relationships into a living, evidence-backed knowledge graph you can search, inspect, export, and hand to an AI agent.";
