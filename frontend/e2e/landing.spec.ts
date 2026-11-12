import { test, expect } from "@playwright/test";

test.describe("Landing showcase", () => {
  test("tells the whole capability story", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { level: 1 })).toContainText(
      "Notes in"
    );
    await expect(
      page.getByRole("link", { name: "Open the canvas" }).first()
    ).toBeVisible();
    await expect(
      page.getByRole("region", { name: "Weave at a glance" })
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Not every note needs a model." })
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "A memory your agent can audit." })
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Every capability, in one place." })
    ).toBeVisible();
  });

  test("interactive sections respond to input", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Prose note" }).click();
    await expect(page.getByText(/bounded llm/i).first()).toBeVisible();

    await page.getByRole("button", { name: "Overview", exact: true }).click();
    await expect(
      page.getByRole("img", { name: /Knowledge graph at the overview level/ })
    ).toBeVisible();

    await page.getByRole("button", { name: "Recall", exact: true }).click();
    await expect(
      page.getByText(/recall_memory\("who worked with Babbage\?"/)
    ).toBeVisible();

    await page.getByRole("button", { name: "Cypher", exact: true }).click();
    await expect(page.getByText(/MERGE \(n:Entity/).first()).toBeVisible();

    await page.getByRole("button", { name: "Agents", exact: true }).click();
    await expect(page.getByText("showing 9 of 58")).toBeVisible();

    await page.getByRole("button", { name: "All", exact: true }).click();
    await expect(page.getByText("showing 58 of 58")).toBeVisible();
  });

  test("mobile navigation exposes the anchors", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await page.getByRole("button", { name: "Open menu" }).click();
    const menu = page.locator("#mobile-nav");
    await expect(menu.getByRole("link", { name: "How it works" })).toBeVisible();

    await menu.getByRole("link", { name: "Canvas", exact: true }).click();
    await expect(page).toHaveURL(/#canvas$/);

    await expect
      .poll(() =>
        page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)
      )
      .toBe(true);
  });
});
