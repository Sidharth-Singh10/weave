#!/usr/bin/env node
/**
 * Public demo smoke test.
 *
 *   node scripts/demo-smoke.mjs https://weave.mutref.tech
 *
 * Exercises the deployed site the way a reviewer's browser does: landing page,
 * demo entry point, a real session, and the API behind the same origin. Exits
 * non-zero on the first failed check.
 *
 * Optional: set API_URL (e.g. https://api.example.com) to also probe the
 * backend's /health/ready, which checks PostgreSQL and Redis. That route is not
 * exposed through the frontend's rewriting rules.
 */

const base = (process.argv[2] || "").replace(/\/+$/, "");
if (!base) {
  console.error("usage: node scripts/demo-smoke.mjs <base-url>");
  process.exit(2);
}

const results = [];

function pass(name, detail = "") {
  results.push({ ok: true, name, detail });
  console.log(`  ok    ${name}${detail ? ` — ${detail}` : ""}`);
}

function fail(name, detail) {
  results.push({ ok: false, name, detail });
  console.log(`  FAIL  ${name}${detail ? ` — ${detail}` : ""}`);
}

function check(name, condition, detail) {
  if (condition) pass(name, detail);
  else fail(name, detail);
}

async function get(path, init) {
  return fetch(`${base}${path}`, { redirect: "manual", ...init });
}

async function main() {
  console.log(`demo smoke: ${base}`);

  const home = await get("/");
  const homeBody = await home.text();
  check("landing page", home.status === 200, `HTTP ${home.status}`);
  check(
    "landing CTA points at /demo",
    homeBody.includes('"/demo"'),
    "NEXT_PUBLIC_DEMO_MODE is baked into the build"
  );

  const demo = await get("/demo");
  await demo.text();
  check("demo entry page", demo.status === 200, `HTTP ${demo.status}`);

  const login = await get("/auth/demo/login", { method: "POST" });
  await login.text();
  const setCookie = login.headers.get("set-cookie") ?? "";
  check("demo login", login.status >= 200 && login.status < 300, `HTTP ${login.status}`);
  check("demo login sets a session cookie", setCookie.includes("weave_session="));

  const cookie = setCookie.split(";")[0];
  const me = await get("/auth/me", { headers: { cookie } });
  const meBody = await me.json().catch(() => ({}));
  check("session is authenticated", meBody.authenticated === true);
  check("demo account is a member", meBody.user?.role === "member", `role=${meBody.user?.role}`);
  check("demo account is not an admin", meBody.user?.role !== "admin");

  const status = await get("/api/status");
  const statusBody = await status.json().catch(() => ({}));
  check("api reachable", status.status === 200, `mode=${statusBody.mode ?? "unknown"}`);

  const apiUrl = (process.env.API_URL || "").replace(/\/+$/, "");
  if (apiUrl) {
    const ready = await fetch(`${apiUrl}/health/ready`).catch(() => null);
    check("api ready (db + redis)", ready?.status === 200, ready ? `HTTP ${ready.status}` : "unreachable");
  }

  const failed = results.filter((r) => !r.ok);
  console.log(
    failed.length === 0
      ? `PASS ${results.length}/${results.length} checks against ${base}`
      : `FAIL ${failed.length}/${results.length} checks failed against ${base}`
  );
  process.exit(failed.length === 0 ? 0 : 1);
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
