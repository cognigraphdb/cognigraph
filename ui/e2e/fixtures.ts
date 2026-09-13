import { randomUUID } from "node:crypto";
import { type APIRequestContext, test as base, expect, type Page } from "@playwright/test";

export const edition = process.env.CG_UI_TEST_EDITION;
const password = process.env.CG_UI_TEST_PASSWORD;
if (!password) throw new Error("Disposable server credentials are required.");

export async function login(page: Page, username = "admin", secret = password as string) {
  await page.getByRole("textbox", { name: "Username", exact: true }).fill(username);
  await page.getByRole("textbox", { name: "Password", exact: true }).fill(secret);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("button", { name: "Sign out", exact: true })).toBeVisible();
}

export async function headers(request: APIRequestContext, username = "admin", secret = password) {
  const response = await request.post("/api/auth/login", {
    data: { username, password: secret },
  });
  expect(response.status()).toBe(200);
  const { token } = await response.json();
  return { Authorization: `Bearer ${token}` };
}

type Fixture = {
  collection: string;
  admin: Record<string, string>;
  diagnostics: { allow: (method: string, path: string, status: number) => void };
};

export const test = base.extend<Fixture>({
  collection: async ({ request }, use) => {
    const collection = `qa_${randomUUID().replaceAll("-", "")}`;
    const admin = await headers(request);
    const response = await request.post("/api/collections", {
      headers: admin,
      data: { name: collection },
    });
    expect(response.status()).toBe(200);
    // The runner removes the entire isolated store even after a failed test.
    await use(collection);
  },
  admin: async ({ request }, use) => {
    await use(await headers(request));
  },
  diagnostics: [
    async ({ page, baseURL }, use, testInfo) => {
      const allowed = new Set(["GET /api/auth/session 401"]);
      const failures: string[] = [];
      page.on("pageerror", (error) => failures.push(error.message));
      page.on("response", (response) => {
        if (response.status() < 400) return;
        const path = new URL(response.url()).pathname;
        const label = `${response.request().method()} ${path} ${response.status()}`;
        if (!allowed.has(label)) failures.push(label);
      });
      page.on("requestfailed", (request) => {
        // Navigating away can cancel an obsolete read. Other transport errors fail.
        if (!request.failure()?.errorText.includes("ERR_ABORTED")) {
          failures.push(`${request.method()} ${new URL(request.url()).pathname} transport failure`);
        }
      });
      // Any attempted external call is a failure, including model providers.
      await page.route("**/*", async (route) => {
        const url = new URL(route.request().url());
        if (["http:", "https:"].includes(url.protocol) && url.origin !== baseURL) {
          failures.push(`Unexpected origin: ${url.origin}`);
          await route.abort();
        } else {
          await route.continue();
        }
      });
      await use({ allow: (method, path, status) => allowed.add(`${method} ${path} ${status}`) });
      if (!page.isClosed()) {
        const path = testInfo.outputPath("final-state.png");
        await page.screenshot({ path, animations: "disabled" });
        await testInfo.attach("final state", { path, contentType: "image/png" });
      }
      expect(failures, "Unexpected browser/API failures").toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };
