import { test, expect, type Page } from "@playwright/test";

interface PersistedNode {
  id: string;
  label: string;
  kind: string;
}

interface PersistedScene {
  version: number;
  nodes: PersistedNode[];
  edges: { id: string; source: string; target: string; relation: string }[];
  positions: Record<string, { x: number; y: number }>;
  viewConfig: { type: string; semanticZoom: string };
}

interface SessionMeta {
  id: string;
  name: string;
}

/** Type a note and submit it through the InputDock. */
async function addNote(page: Page, text: string) {
  const input = page.getByRole("textbox", { name: "Add a note to the graph" });
  await input.fill(text);
  await input.press("Enter");
}

/** Read the active session's persisted scene blob. */
async function activeScene(page: Page): Promise<PersistedScene> {
  return page.evaluate(() => {
    const active = localStorage.getItem("weave:active-session");
    const raw = active ? localStorage.getItem(`weave:session:${active}`) : null;
    return JSON.parse(raw ?? "{}") as PersistedScene;
  });
}

async function sessionNames(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const raw = localStorage.getItem("weave:sessions");
    return (JSON.parse(raw ?? "[]") as SessionMeta[]).map((s) => s.name);
  });
}

/** Open the session switcher dropdown. The menu stays open after a menuitem
 * click (only "New session"/"Reset" close it), so force-close via an outside
 * click before toggling the button. */
async function openMenu(page: Page) {
  if (await page.getByRole("menu").count()) {
    await page.locator(".react-flow__pane").first().click();
  }
  await page.getByRole("button", { name: "Switch session" }).click();
  await expect(page.getByRole("menu")).toBeVisible();
}

const switchBtn = (page: Page) =>
  page.getByRole("button", { name: "Switch session" });

test.describe("Excalidraw-style persistent sessions", () => {
  test("bootstraps a default session on first visit", async ({ page }) => {
    await page.goto("/app");
    await expect(switchBtn(page)).toContainText("Session 1");

    const state = await page.evaluate(() => {
      const sessions = JSON.parse(
        localStorage.getItem("weave:sessions") ?? "[]"
      ) as SessionMeta[];
      const active = localStorage.getItem("weave:active-session");
      const scene = JSON.parse(
        (active ? localStorage.getItem(`weave:session:${active}`) : null) ?? "{}"
      ) as PersistedScene;
      return {
        sessions,
        active,
        nodes: scene.nodes,
        edges: scene.edges,
        version: scene.version,
      };
    });

    expect(state.sessions).toHaveLength(1);
    expect(state.sessions[0].name).toBe("Session 1");
    expect(state.active).toBe(state.sessions[0].id);
    expect(state.nodes).toEqual([]);
    expect(state.edges).toEqual([]);
    expect(state.version).toBe(1);
  });

  test("auto-saves graph changes to localStorage after the debounce", async ({
    page,
  }) => {
    await page.goto("/app");
    await addNote(page, "Ron is afraid of spiders.");
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();

    await expect
      .poll(async () => (await activeScene(page)).nodes.map((n) => n.label))
      .toContain("spiders");

    const scene = await activeScene(page);
    expect(scene.nodes.map((n) => n.label)).toEqual(
      expect.arrayContaining(["Ron", "spiders"])
    );
    expect(scene.edges.some((e) => e.relation === "afraid of")).toBe(true);
    expect(Object.keys(scene.positions)).toEqual(
      expect.arrayContaining(["node-ron", "node-spiders"])
    );
  });

  test("restores the saved session on reload and shows a toast", async ({
    page,
  }) => {
    await page.goto("/app");
    await addNote(page, "Ron is afraid of spiders.");
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(page)).nodes.length)
      .toBeGreaterThan(0);

    await page.reload();

    await expect(page.getByText("Restored Session 1")).toBeVisible();
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    // The toast auto-dismisses after ~3.5s.
    await expect(page.getByText("Restored Session 1")).toBeHidden();
  });

  test("session switcher: create, isolate, rename, delete, reset", async ({
    page,
  }) => {
    await page.goto("/app");
    await expect(switchBtn(page)).toContainText("Session 1");

    // Seed session 1 with a graph.
    await addNote(page, "Ron is afraid of spiders.");
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(page)).nodes.length)
      .toBeGreaterThan(0);

    // Create a new session -> fresh, empty canvas.
    await openMenu(page);
    await page.getByRole("button", { name: "New session" }).click();
    await expect(switchBtn(page)).toContainText("Session 2");
    await expect(page.getByText("Your canvas is empty.")).toBeVisible();
    await expect(page.getByText("spiders", { exact: true })).toHaveCount(0);

    // Seed session 2 with a different graph.
    await addNote(page, "Hermione studies at Hogwarts.");
    await expect(page.getByText("Hermione", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(page)).nodes.map((n) => n.label))
      .toContain("Hermione");

    // Switch back to session 1 -> graphs are isolated.
    await openMenu(page);
    await page.getByRole("menuitem", { name: /^Session 1/ }).click();
    await expect(switchBtn(page)).toContainText("Session 1");
    await expect(page.getByText("Ron", { exact: true })).toBeVisible();
    await expect(page.getByText("Hermione", { exact: true })).toHaveCount(0);

    // Switch to session 2 and rename it via prompt.
    await openMenu(page);
    await page.getByRole("menuitem", { name: /^Session 2/ }).click();
    await openMenu(page);
    page.once("dialog", (d) => d.accept("Work"));
    await page.getByRole("button", { name: "Rename Session 2" }).click();
    await expect(page.getByRole("menuitem", { name: /^Work/ })).toBeVisible();

    // Delete the renamed session via confirm; active falls back to Session 1.
    page.once("dialog", (d) => d.accept());
    await page.getByRole("button", { name: "Delete Work" }).click();
    await expect(page.getByRole("menuitem", { name: /^Work/ })).toHaveCount(0);
    await expect(switchBtn(page)).toContainText("Session 1");
    await expect(await sessionNames(page)).toEqual(["Session 1"]);

    // Reset clears the graph but keeps the session.
    await page.locator(".react-flow__pane").first().click();
    await addNote(page, "Hogwarts has four houses.");
    await expect(page.getByText("Hogwarts", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(page)).nodes.length)
      .toBeGreaterThan(0);
    await openMenu(page);
    page.once("dialog", (d) => d.accept());
    await page.getByRole("button", { name: "Reset session" }).click();
    await expect(page.getByText("Your canvas is empty.")).toBeVisible();
    await expect(switchBtn(page)).toContainText("Session 1");
    await expect(await sessionNames(page)).toEqual(["Session 1"]);
  });

  test("syncs graph changes across tabs via storage events", async ({
    browser,
  }) => {
    const context = await browser.newContext();
    const pageA = await context.newPage();
    await pageA.goto("/app");
    await expect(switchBtn(pageA)).toContainText("Session 1");

    // Seed the graph in tab A.
    await addNote(pageA, "Harry Potter lives in London.");
    await expect(pageA.getByText("London", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(pageA)).nodes.length)
      .toBeGreaterThan(0);

    // Tab B hydrates the same graph from shared localStorage.
    const pageB = await context.newPage();
    await pageB.goto("/app");
    await expect(pageB.getByText("Harry Potter", { exact: true })).toBeVisible();

    // Edit in tab A; tab B picks it up live (storage event), no reload.
    await addNote(pageA, "Hogwarts has four houses.");
    await expect(pageA.getByText("houses", { exact: true })).toBeVisible();
    await expect(pageB.getByText("houses", { exact: true })).toBeVisible({
      timeout: 10_000,
    });

    await context.close();
  });

  test("shows the quota-exceeded banner when localStorage is full", async ({
    page,
  }) => {
    await page.goto("/app");
    await expect(switchBtn(page)).toContainText("Session 1");

    // Fill localStorage down to the last few bytes so any scene growth fails.
    await page.evaluate(() => {
      const block = "x".repeat(64 * 1024);
      let i = 0;
      try {
        while (true) localStorage.setItem(`pad-${i++}`, block);
      } catch {}
      const small = "y".repeat(64);
      let j = 0;
      try {
        while (true) localStorage.setItem(`pad-s-${j++}`, small);
      } catch {}
      let k = 0;
      try {
        while (true) localStorage.setItem(`z${k++}`, "1");
      } catch {}
    });

    await addNote(page, "Ron is afraid of spiders.");
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    await expect(
      page.getByText("Storage full — the latest changes may not be saved.")
    ).toBeVisible();
  });

  test("persists the selected view across reloads", async ({ page }) => {
    await page.goto("/app");
    await page.getByRole("button", { name: "Topic" }).click();

    await expect
      .poll(async () => (await activeScene(page)).viewConfig.type)
      .toBe("topic");

    await page.reload();

    await expect(page.getByRole("button", { name: "Topic" })).toHaveClass(
      /bg-accent/
    );
  });

  test("flushes pending changes on reload before the debounce fires", async ({
    page,
  }) => {
    await page.goto("/app");
    await addNote(page, "Ron is afraid of spiders.");
    // Reload as soon as the node renders — inside the 300ms debounce window —
    // so the beforeunload flush is the only thing that can persist it.
    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    await page.reload();

    await expect(page.getByText("spiders", { exact: true })).toBeVisible();
    await expect
      .poll(async () => (await activeScene(page)).nodes.map((n) => n.label))
      .toContain("spiders");
  });
});
