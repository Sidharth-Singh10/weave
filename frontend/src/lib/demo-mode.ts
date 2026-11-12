/** Build-time switch for the public, sign-in-free demo. */
export const demoModeEnabled = process.env.NEXT_PUBLIC_DEMO_MODE === "true";

/**
 * Where the primary calls to action should send a visitor. In demo mode the
 * canvas is reached through `/demo`, which mints a session first; otherwise the
 * canvas itself handles sign-in.
 */
export const canvasEntryHref = demoModeEnabled ? "/demo" : "/app";
