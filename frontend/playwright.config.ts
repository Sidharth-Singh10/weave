import { defineConfig, devices } from "@playwright/test";

const PORT = 3000;
const API_PORT = 3001;
const BASE_URL = `http://localhost:${PORT}`;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      // Deterministic mock extractor: unset OPENCODE_API_KEY so the backend
      // never talks to a real LLM during tests.
      command: "env -u OPENCODE_API_KEY ../backend/target/debug/weave-api",
      url: `http://localhost:${API_PORT}/health`,
      reuseExistingServer: !process.env.CI,
      timeout: 60_000,
    },
    {
      command: "npm run dev",
      url: BASE_URL,
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
  ],
});
