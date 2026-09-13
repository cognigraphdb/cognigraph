import { expect, login, test } from "./fixtures.ts";

test("CodeMirror edits execute CGQL and Lua against the real server", async ({ page }) => {
  await page.goto("/query");
  await login(page);
  const query = 'RETURN { label: "Café β", value: 7 }';
  await page.getByRole("textbox", { name: "CGQL query", exact: true }).fill(query);
  const queryResponse = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/search/query") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Run query", exact: true }).click();
  const queried = await queryResponse;
  expect(queried.status()).toBe(200);
  expect(queried.request().postDataJSON().query).toBe(query);
  await expect(page.locator(".result-body")).toContainText("Café β");

  await page.getByRole("link", { name: "Lua", exact: true }).click();
  const script = "return graph.query('RETURN { label: \"Lua β\", value: 8 }')";
  await page.getByRole("textbox", { name: "Lua script", exact: true }).fill(script);
  const luaResponse = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/lua/execute") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Run script", exact: true }).click();
  const executed = await luaResponse;
  expect(executed.status()).toBe(200);
  expect(executed.request().postDataJSON().script).toBe(script);
  await expect(page.locator(".result-body")).toContainText("Lua β");
});
