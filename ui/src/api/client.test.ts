import { afterEach, describe, expect, mock, test } from "bun:test";
import { CogniGraphApi } from "./client.ts";

const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});

function respond(status: number, body: unknown, contentType = "application/json") {
  const fetchMock = mock(
    async (_url: RequestInfo | URL, _init?: RequestInit) =>
      new Response(typeof body === "string" ? body : JSON.stringify(body), {
        status,
        headers: { "Content-Type": contentType },
      }),
  );
  globalThis.fetch = fetchMock as unknown as typeof fetch;
  return fetchMock;
}
const api = (token = "") => new CogniGraphApi({ baseUrl: "http://127.0.0.1:38471", token });

test("health carries only recognized product editions for provisioning choices", async () => {
  for (const edition of ["community", "enterprise", "unknown", undefined]) {
    respond(200, { status: "ok", database: "connected", edition });
    expect((await api().health()).edition).toBe(
      edition === "community" || edition === "enterprise" ? edition : undefined,
    );
  }
});

describe("authentication probe", () => {
  test("verifies host-admin through the tenant catalog after data access is denied", async () => {
    const urls: string[] = [];
    globalThis.fetch = mock(async (url: string) => {
      urls.push(url);
      return url.endsWith("/collections")
        ? Response.json({ error: "Forbidden" }, { status: 403 })
        : Response.json({ tenants: [], count: 0 });
    }) as unknown as typeof fetch;
    expect(await api("synthetic-host-token").authRequired()).toBe(false);
    expect(urls).toEqual([
      "http://127.0.0.1:38471/api/collections",
      "http://127.0.0.1:38471/api/tenants",
    ]);
  });
  test("the tenant fallback must itself verify access and propagate expiry", async () => {
    for (const fallback of [
      () => Response.json({ tenants: "invalid" }),
      () => Response.json({ error: "Unavailable" }, { status: 503 }),
      () => Promise.reject(new TypeError("Failed to fetch")),
    ]) {
      globalThis.fetch = mock(async (url: string) =>
        url.endsWith("/collections")
          ? Response.json({ error: "Forbidden" }, { status: 403 })
          : fallback(),
      ) as unknown as typeof fetch;
      await expect(api().authRequired()).rejects.toThrow();
    }
    globalThis.fetch = mock(async (url: string) =>
      Response.json({ error: "Denied" }, { status: url.endsWith("/collections") ? 403 : 401 }),
    ) as unknown as typeof fetch;
    const client = api("synthetic-expired-token");
    const expired = mock(() => {});
    client.onUnauthorized = expired;
    expect(await client.authRequired()).toBe(true);
    expect(expired).toHaveBeenCalledTimes(1);
  });
  test("accepts a verified empty collection list on the configured origin", async () => {
    const fetchMock = respond(200, { collections: [] });
    expect(await api().authRequired()).toBe(false);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://127.0.0.1:38471/api/collections");
  });
  test("401 requests login and invalidates an expired bearer session", async () => {
    respond(401, { error: "Authentication required" });
    const client = api("synthetic-token");
    const expired = mock(() => {});
    client.onUnauthorized = expired;
    expect(await client.authRequired()).toBe(true);
    expect(expired).toHaveBeenCalledTimes(1);
  });
  test("denied, missing, limited, and failing endpoints do not imply anonymous access", async () => {
    for (const status of [403, 404, 429, 500, 503]) {
      respond(status, { error: "Unavailable" });
      await expect(api().authRequired()).rejects.toThrow("Unavailable");
    }
  });
  test("an HTML fallback or malformed JSON shape cannot open the console", async () => {
    for (const body of [
      "<!doctype html><title>UI</title>",
      null,
      [],
      {},
      { collections: "wrong" },
    ]) {
      respond(200, body);
      await expect(api().authRequired()).rejects.toThrow("valid CogniGraph collections response");
    }
  });
  test("connection failure and timeout propagate, with a bounded probe signal", async () => {
    let signal: AbortSignal | undefined;
    globalThis.fetch = mock(async (_url: unknown, init?: RequestInit) => {
      signal = init?.signal ?? undefined;
      throw new TypeError("Failed to fetch");
    }) as unknown as typeof fetch;
    await expect(api().authRequired()).rejects.toThrow("Failed to fetch");
    expect(signal).toBeInstanceOf(AbortSignal);
    globalThis.fetch = mock(async () => {
      throw new DOMException("Probe timed out", "TimeoutError");
    }) as unknown as typeof fetch;
    await expect(api().authRequired()).rejects.toThrow("Probe timed out");
  });
});
