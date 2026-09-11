import { randomUUID } from "node:crypto";
import { edition, expect, headers, login, test } from "./fixtures.ts";

test("Viewer has read access while UI and real API deny mutation and administration", async ({
  page,
  request,
  admin,
  collection,
}) => {
  const username = `viewer-${randomUUID()}`;
  const secret = randomUUID();
  expect(
    (
      await request.post("/api/users", {
        headers: admin,
        data: { username, password: secret, role: "viewer" },
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await request.post("/api/documents", {
        headers: admin,
        data: { collection, _key: "read-only", text: "Visible to Viewer" },
      })
    ).status(),
  ).toBe(200);
  await page.goto(`/collections/${collection}?doc=read-only`);
  await login(page, username, secret);
  await expect(page.locator(".inspector")).toContainText("Visible to Viewer");
  for (const name of ["Create document", "Edit", "Delete"]) {
    await expect(page.getByRole("button", { name, exact: true })).toBeDisabled();
  }
  await expect(page.getByRole("link", { name: "Users", exact: true })).toHaveCount(0);
  const viewer = await headers(request, username, secret);
  expect(
    (await request.get(`/api/documents/${collection}/read-only`, { headers: viewer })).status(),
  ).toBe(200);
  expect(
    (
      await request.patch(`/api/documents/${collection}/read-only`, {
        headers: viewer,
        data: { text: "must not persist" },
      })
    ).status(),
  ).toBe(403);
  expect((await request.get("/api/users", { headers: viewer })).status()).toBe(403);
  const stored = await request.get(`/api/documents/${collection}/read-only`, { headers: admin });
  expect(await stored.json()).toMatchObject({ text: "Visible to Viewer" });
  await page.goto("/users");
  await expect(page.getByText("Page unavailable", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Open your workspace" }).click();
  await expect(page).toHaveURL(/\/collections$/);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await page.reload();
  await expect(page.getByRole("heading", { name: "CogniGraph Console" })).toBeVisible();
});

test("edition-specific review availability and accessible collapsed navigation", async ({
  page,
  diagnostics,
}) => {
  await page.goto("/");
  await login(page);
  await expect(page).toHaveURL(/\/collections$/);
  await page.setViewportSize({ width: 1067, height: 667 });
  await page.getByRole("link", { name: "Operations", exact: true }).press("Enter");
  await expect(page.getByRole("link", { name: "Operations", exact: true })).toHaveAttribute(
    "aria-current",
    "page",
  );
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const review = page.getByRole("link", { name: "Review", exact: true });
  if (edition === "community") {
    await expect(review).toHaveCount(0);
    await page.goto("/review");
    await expect(
      page.getByText("This page requires an Enterprise server.", { exact: true }),
    ).toBeVisible();
  } else {
    diagnostics.allow("GET", "/api/documents", 404);
    await review.press("Enter");
    await expect(page.getByRole("heading", { name: "Review", exact: true })).toBeVisible();
    await expect(review).toHaveAttribute("aria-current", "page");
    await expect(review).toHaveClass(/\bactive\b/);
    await expect(page.getByRole("link", { name: "Operations", exact: true })).not.toHaveClass(
      /\bactive\b/,
    );
    await expect(
      page.getByText("No accepted spaces in this tenant", { exact: false }),
    ).toBeVisible();
  }
});

if (edition === "enterprise") {
  test("HostAdmin lands on Tenants and cannot access tenant documents", async ({
    page,
    request,
  }) => {
    const secret = process.env.CG_UI_TEST_HOST_PASSWORD;
    if (!secret) throw new Error("Host bootstrap credentials are required.");
    await page.goto("/");
    await login(page, "host-admin", secret);
    await expect(page).toHaveURL(/\/tenants$/);
    await expect(page.getByRole("heading", { name: "Tenants", exact: true })).toBeVisible();
    await expect(page.getByRole("navigation").getByRole("link")).toHaveCount(2);
    const host = await headers(request, "host-admin", secret);
    expect((await request.get("/api/tenants", { headers: host })).status()).toBe(200);
    expect((await request.get("/api/collections", { headers: host })).status()).toBe(403);
    await page.goto("/collections");
    await expect(page.getByText("Page unavailable", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Open your workspace" }).click();
    await page.reload();
    await expect(page.getByRole("heading", { name: "Tenants", exact: true })).toBeVisible();
  });
}
