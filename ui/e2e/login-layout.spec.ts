import type { Locator, Page } from "@playwright/test";
import { expect, login, test } from "./fixtures.ts";

async function expectCentered(page: Page) {
  const card = await page.locator(".auth-card").boundingBox();
  const viewport = page.viewportSize();
  if (!card || !viewport) throw new Error("Login geometry is unavailable");
  expect(Math.abs(card.x + card.width / 2 - viewport.width / 2)).toBeLessThanOrEqual(1);
  expect(Math.abs(card.y + card.height / 2 - viewport.height / 2)).toBeLessThanOrEqual(1);
  expect(card.x).toBeGreaterThanOrEqual(0);
  expect(card.x + card.width).toBeLessThanOrEqual(viewport.width);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(viewport.width);
}

async function expectReachable(page: Page, control: Locator) {
  await control.scrollIntoViewIfNeeded();
  const box = await control.boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error("Control geometry is unavailable");
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
}

test("login fits narrow viewports while the authenticated workspace keeps its desktop width", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "CogniGraph Console" })).toBeVisible();
  for (const [width, height] of [
    [1600, 1000],
    [1280, 800],
    [1067, 667],
    [960, 800],
    [800, 800],
    [640, 800],
    [390, 844],
    [320, 568],
  ] as const) {
    await page.setViewportSize({ width, height });
    await expectCentered(page);
  }
  await expect(page.getByRole("textbox", { name: "Password", exact: true })).toHaveCSS(
    "font-size",
    "16px",
  );

  await login(page);
  await expect(page.locator(".app-shell")).toHaveCSS("min-width", "960px");
  expect((await page.locator(".app-shell").boundingBox())?.width).toBe(960);
  await page.setViewportSize({ width: 1280, height: 800 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(1280);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await expectCentered(page);
});

test("short login viewports can scroll through validation, errors and a long server name", async ({
  page,
  diagnostics,
}) => {
  await page.setViewportSize({ width: 390, height: 280 });
  await page.goto("/");
  const submit = page.getByRole("button", { name: "Sign in", exact: true });
  await submit.click();
  await expect(page.getByText("Enter your username.", { exact: true })).toBeVisible();
  await expectReachable(page, page.getByText("Enter your username.", { exact: true }));
  await expectReachable(page, page.getByText("Enter your password.", { exact: true }));
  await expectReachable(page, submit);

  diagnostics.allow("POST", "/api/auth/login", 401);
  await page.getByRole("textbox", { name: "Username", exact: true }).fill("admin");
  await page.getByRole("textbox", { name: "Password", exact: true }).fill("incorrect-layout-probe");
  await submit.click();
  const error = page.getByText("Invalid username or password.", { exact: true });
  await expect(error).toBeVisible();
  await expectReachable(page, error);
  await expectReachable(page, submit);

  // Presentation-only stress fixture: the real API origin and responses stay local.
  await page.locator(".auth-foot strong").evaluate((element) => {
    element.textContent = `${"longservername".repeat(8)}.example.test`;
  });
  await expectReachable(page, page.locator(".auth-foot"));
  const screen = await page.locator(".auth-screen").evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
  }));
  expect(screen.scrollWidth).toBe(screen.clientWidth);
  expect(screen.scrollHeight).toBeGreaterThan(screen.clientHeight);
  await expectReachable(page, page.getByRole("textbox", { name: "Username", exact: true }));
});
