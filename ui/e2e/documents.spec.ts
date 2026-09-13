import { expect, login, test } from "./fixtures.ts";

test("fresh production origin preserves deep links, encoded keys and browser history", async ({
  page,
  request,
  admin,
  collection,
  baseURL,
  diagnostics,
}) => {
  for (let index = 0; index < 31; index++) {
    const response = await request.post("/api/documents", {
      headers: admin,
      data: { collection, _key: index === 30 ? "last+key" : `item-${index}`, ordinal: index },
    });
    expect(response.status()).toBe(200);
  }
  const path = `/collections/${collection}?doc=last%2Bkey`;
  const response = await page.goto(path);
  expect(response?.status()).toBe(200);
  expect(response?.headers()["content-type"]).toContain("text/html");
  await expect(page.getByRole("heading", { name: "CogniGraph Console" })).toBeVisible();
  expect(page.url()).toBe(baseURL + path);
  diagnostics.allow("POST", "/api/auth/login", 401);
  await page.getByRole("textbox", { name: "Username", exact: true }).fill("admin");
  await page.getByRole("textbox", { name: "Password", exact: true }).fill("incorrect");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Invalid username or password");
  await login(page);
  await expect(page.locator(".inspector")).toContainText(`"_key": "last+key"`);
  await expect(page.getByRole("textbox", { name: "Search documents" })).toHaveValue("last+key");
  expect(page.url()).toBe(`${baseURL}/collections/${collection}`);
  await page.getByRole("link", { name: "Query", exact: true }).press("Enter");
  await expect(page.getByRole("heading", { name: "Query console" })).toBeVisible();
  await page.getByRole("link", { name: "Lua", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Lua console" })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("link", { name: "Query", exact: true })).toHaveAttribute(
    "aria-current",
    "page",
  );
  await page.goForward();
  await expect(page.getByRole("link", { name: "Lua", exact: true })).toHaveAttribute(
    "aria-current",
    "page",
  );
  await page.goto(`/collections/${collection}`);
  await page.reload();
  await expect(page.getByRole("heading", { name: collection, exact: true })).toBeVisible();
  // A new context has no saved API override: all requests must use this random origin.
  expect(await page.evaluate(() => Object.keys(localStorage))).toEqual([]);
});

test("document creation, raw JSON edit and confirmed deletion persist through reload", async ({
  page,
  request,
  admin,
  collection,
}) => {
  await page.goto(`/collections/${collection}`);
  await login(page);
  await page.getByRole("button", { name: "Create document", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("textbox", { name: "Title", exact: true }).fill("Synthetic round trip");
  await dialog.getByRole("textbox", { name: /^Summary/ }).fill("Initial document");
  const created = page.waitForResponse(
    (r) => r.url().endsWith("/api/documents") && r.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "Create document", exact: true }).click();
  const createResponse = await created;
  expect(createResponse.status()).toBe(200);
  const { _key: key } = await createResponse.json();
  const read = () => request.get(`/api/documents/${collection}/${key}`, { headers: admin });
  let response = await read();
  expect(response.status()).toBe(200);
  expect(await response.json()).toMatchObject({
    title: "Synthetic round trip",
    summary: "Initial document",
  });
  await expect(page.locator(".inspector")).toContainText(key);
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  const editor = page.getByRole("textbox", { name: "Document JSON" });
  const draft = JSON.parse(await editor.inputValue());
  const fields = {
    title: ["Structured", { nested: true }],
    summary: 42,
    nullable: null,
    extension: { enabled: false, values: [0, "", null, { unicode: "Δ" }] },
  };
  await editor.fill(JSON.stringify({ ...draft, ...fields }));
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(editor).toHaveCount(0);
  response = await read();
  expect(await response.json()).toMatchObject({ _key: key, ...fields });
  await page.reload();
  await page.getByRole("button", { name: key, exact: true }).click();
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  expect(JSON.parse(await editor.inputValue())).toMatchObject(fields);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(dialog).toContainText(collection);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  expect((await read()).status()).toBe(200);
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await dialog.getByRole("button", { name: "Delete document", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect((await read()).status()).toBe(404);
  await page.reload();
  await expect(page.getByRole("heading", { name: collection, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: key, exact: true })).toHaveCount(0);
});
