import { afterEach, describe, expect, mock, test } from "bun:test";
import type { SessionContext } from "../lib/access.ts";
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

describe("verified session context", () => {
  const context: SessionContext = {
    auth_enabled: true,
    edition: "enterprise",
    scopes: ["promotion-read", "policy-author"],
    user: { key: "qa", username: "qa-author", role: "policy-author", tenant: "qa-tenant" },
  };
  test("verifies governance identity without needing a tenant-data catalog", async () => {
    const fetchMock = respond(200, context);
    expect(await api("synthetic-token").sessionContext()).toEqual(context);
    expect(fetchMock.mock.calls).toHaveLength(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://127.0.0.1:38471/api/auth/session");
  });
  test("accepts explicit anonymous mode without inventing a role", async () => {
    const anonymous: SessionContext = {
      auth_enabled: false,
      edition: "community",
      scopes: [],
      user: null,
    };
    respond(200, anonymous);
    expect(await api().sessionContext()).toEqual(anonymous);
  });
  test("401 invalidates an expired bearer session", async () => {
    respond(401, { error: "Authentication required" });
    const client = api("synthetic-token");
    const expired = mock(() => {});
    client.onUnauthorized = expired;
    await expect(client.sessionContext()).rejects.toThrow("Authentication required");
    expect(expired).toHaveBeenCalledTimes(1);
  });
  test("denied, missing, limited, and failing endpoints never imply anonymous access", async () => {
    for (const status of [403, 404, 429, 500, 503]) {
      respond(status, { error: "Unavailable" });
      await expect(api().sessionContext()).rejects.toThrow("Unavailable");
    }
  });
  test("HTML, unverified identity and malformed scope/edition responses stay closed", async () => {
    for (const body of [
      "<!doctype html><title>UI</title>",
      null,
      [],
      {},
      { collections: [] },
      { ...context, edition: "unknown" },
      { ...context, user: null },
      { ...context, user: { ...context.user, tenant: "" } },
      { ...context, scopes: "admin" },
      { ...context, scopes: [1] },
      { ...context, auth_enabled: false },
      { ...context, auth_enabled: false, user: null },
    ]) {
      respond(200, body);
      await expect(api().sessionContext()).rejects.toThrow("valid CogniGraph session response");
    }
  });
  test("connection failure and timeout propagate, with a bounded probe signal", async () => {
    let signal: AbortSignal | undefined;
    globalThis.fetch = mock(async (_url: unknown, init?: RequestInit) => {
      signal = init?.signal ?? undefined;
      throw new TypeError("Failed to fetch");
    }) as unknown as typeof fetch;
    await expect(api().sessionContext()).rejects.toThrow("Failed to fetch");
    expect(signal).toBeInstanceOf(AbortSignal);
    globalThis.fetch = mock(async () => {
      throw new DOMException("Probe timed out", "TimeoutError");
    }) as unknown as typeof fetch;
    await expect(api().sessionContext()).rejects.toThrow("Probe timed out");
  });
});
