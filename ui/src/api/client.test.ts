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

describe("authentication probe", () => {
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
