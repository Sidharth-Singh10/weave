import { test, expect } from "@playwright/test";
import { loginAndGoto } from "./auth-helpers";

test.describe("Authentication", () => {
  test("unauthenticated visitors are sent to /login", async ({ page }) => {
    await page.goto("/app");
    await expect(page).toHaveURL(/\/login\?next=/);
    await expect(page.getByRole("link", { name: "Continue with Google" })).toBeVisible();
  });

  test("signing in reaches the canvas and persists the session", async ({ page }) => {
    await loginAndGoto(page);
    await expect(page.getByLabel("Add a note to the graph")).toBeVisible();
    // A reload keeps the session (cookie-based, not localStorage).
    await page.reload();
    await expect(page.getByLabel("Add a note to the graph")).toBeVisible();
  });

  test("logout returns to the login page", async ({ page }) => {
    await loginAndGoto(page);
    await page.getByRole("button", { name: "Account menu" }).click();
    await page.getByRole("button", { name: "Logout" }).click();
    await expect(page).toHaveURL(/\/login/);
    await expect(page.getByRole("link", { name: "Continue with Google" })).toBeVisible();
  });

  test("members do not see the admin dashboard link", async ({ page }) => {
    await loginAndGoto(page);
    await page.getByRole("button", { name: "Account menu" }).click();
    await expect(page.getByRole("button", { name: "Logout" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Admin Dashboard" })).toHaveCount(0);
  });

  test("admins see the admin dashboard and reach /admin", async ({ page }) => {
    await loginAndGoto(page, "/app", "owner@example.com");
    await page.getByRole("button", { name: "Account menu" }).click();
    await page.getByRole("link", { name: "Admin Dashboard" }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  });

  test("a member navigating to /admin is redirected away", async ({ page }) => {
    await loginAndGoto(page, "/admin", "member@example.com");
    await expect(page).toHaveURL(/\/login/, { timeout: 10_000 });
  });
});
