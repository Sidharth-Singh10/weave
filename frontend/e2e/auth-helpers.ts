import type { Page } from "@playwright/test";

/**
 * Sign in via the dev-only stub (AUTH_STUB=true) and land on `path`.
 * Uses the browser context's shared request jar, so the HttpOnly session
 * cookie is seeded before navigation (deterministic, no fetch-in-page race).
 */
export async function loginAndGoto(page: Page, path = "/app", email = "e2e@test.com") {
  const res = await page.context().request.post("/auth/test/login", {
    data: { email },
  });
  if (!res.ok()) {
    throw new Error(`stub login failed: ${res.status()} ${await res.text()}`);
  }
  await page.goto(path, { waitUntil: "domcontentloaded" });
}
