import { expect, login, test } from "./fixtures.ts";

test("graph traversal renders, resizes and remounts with the current G6 dependencies", async ({
  page,
  request,
  collection,
  admin,
}) => {
  const errors: string[] = [];
  page.on("console", (message) => {
    // The shared fixture checks HTTP failures with a route-specific allowlist
    // for the expected unauthenticated session probe. Retain other console errors.
    if (message.type() === "error" && !message.text().startsWith("Failed to load resource:")) {
      errors.push(message.text());
    }
  });
  for (const key of ["source", "target"]) {
    const response = await request.post("/api/documents", {
      headers: admin,
      data: { collection, _key: key, content: `Synthetic ${key} evidence` },
    });
    expect(response.status()).toBe(200);
  }
  const edge = await request.post("/api/graph/relationships", {
    headers: admin,
    data: { from: `${collection}/source`, to: `${collection}/target`, relation_type: "SUPPORTS" },
  });
  expect(edge.status()).toBe(200);

  await page.goto("/graph");
  await login(page);
  await page
    .getByPlaceholder("collection/key — e.g. labels/000ae256…")
    .fill(`${collection}/source`);
  const traversal = page.waitForResponse((response) =>
    response.url().endsWith("/api/graph/traverse"),
  );
  await page.getByRole("button", { name: "Run traversal", exact: true }).click();
  const response = await traversal;
  expect(response.status()).toBe(200);
  expect(await response.json()).toMatchObject({ count: 1 });
  const visualization = page.getByRole("application", { name: "Interactive graph visualization" });
  const painted = async () => {
    await expect(visualization).toBeVisible();
    await expect
      .poll(() =>
        visualization.evaluate((element) => {
          // G6 also creates a transparent hit-test layer; inspect the painted
          // main-size canvas rather than depending on layer insertion order.
          for (const canvas of element.querySelectorAll("canvas")) {
            const bounds = canvas.getBoundingClientRect();
            if (bounds.width < 200 || bounds.height < 100) continue;
            const pixels = canvas.getContext("2d")?.getImageData(0, 0, canvas.width, canvas.height);
            if (!pixels) continue;
            const colors = new Set<number>();
            for (let i = 0; i < pixels.data.length; i += 4) {
              if (pixels.data[i + 3]) {
                colors.add(
                  ((pixels.data[i] ?? 0) << 16) |
                    ((pixels.data[i + 1] ?? 0) << 8) |
                    (pixels.data[i + 2] ?? 0),
                );
                if (colors.size > 8) return colors.size;
              }
            }
          }
          return 0;
        }),
      )
      .toBeGreaterThan(8);
  };
  await painted();
  await page.setViewportSize({ width: 1067, height: 667 });
  await page.getByRole("button", { name: "Fit graph to view", exact: true }).click();
  await painted();
  const view = page.getByRole("radiogroup", { name: "Graph view" });
  await view.getByText("JSON", { exact: true }).click();
  await expect(view.getByRole("radio", { name: "JSON", exact: true })).toBeChecked();
  await expect(page.locator(".graph-json-result")).toContainText(`${collection}/target`);
  await view.getByText("Visual", { exact: true }).click();
  await expect(view.getByRole("radio", { name: "Visual", exact: true })).toBeChecked();
  await painted();
  expect(errors).toEqual([]);
});
