import { describe, expect, test } from "bun:test";
import {
  BadRequestError,
  CogniGraph,
  CogniGraphError,
  ConflictError,
  ForbiddenError,
  NetworkError,
  NotFoundError,
  ProtocolError,
  RateLimitError,
  ServerError,
  TimeoutError,
  UnavailableError,
  UniqueViolationError,
} from "../../src/index.js";
import { fakeFetch } from "./fake.js";

const BASE = "http://db.test:3000/";

function client(...replies: Parameters<typeof fakeFetch>) {
  const fake = fakeFetch(...replies);
  const db = new CogniGraph({ baseUrl: BASE, token: "t0k", fetch: fake.fetch });
  return { db, calls: fake.calls, fetch: fake.fetch };
}

const ok = (json: unknown) => ({ status: 200, json });

/** The error a promise rejects with; fails the test if it resolves. */
async function rejection<E>(promise: Promise<unknown>): Promise<E> {
  try {
    await promise;
  } catch (error) {
    return error as E;
  }
  throw new Error("expected the call to reject");
}

describe("requests", () => {
  test("query and mutate send CGQL with bind_vars and a bearer token", async () => {
    const { db, calls } = client(ok({ results: [{ a: 1 }], count: 1 }));
    const rows = await db.query<{ a: number }>("FOR d IN notes FILTER d.n > @n RETURN d", { n: 2 });
    expect(rows).toEqual([{ a: 1 }]);
    expect(calls[0]).toMatchObject({
      method: "POST",
      url: "http://db.test:3000/api/search/query",
      headers: { authorization: "Bearer t0k", "content-type": "application/json" },
      body: { query: "FOR d IN notes FILTER d.n > @n RETURN d", bind_vars: { n: 2 } },
    });
    await db.mutate("INSERT {} INTO notes");
    expect(calls[1]).toMatchObject({
      url: "http://db.test:3000/api/query",
      body: { query: "INSERT {} INTO notes", bind_vars: {} },
    });
  });

  test("document calls encode every key character ArangoDB allows", async () => {
    const key = "c_5(x)+,=;$!*'%:@.";
    const { db, calls } = client(ok({ _key: key }));
    await db.documents.get("customers", key);
    expect(calls[0]?.url).toBe(
      `http://db.test:3000/api/documents/customers/${encodeURIComponent(key)}`,
    );
    await db.documents.create("notes", { _key: "n1", title: "x" });
    expect(calls[1]).toMatchObject({
      method: "POST",
      url: "http://db.test:3000/api/documents",
      body: { collection: "notes", _key: "n1", title: "x" },
    });
    await db.documents.update("notes", "n1", { title: "y" });
    await db.documents.replace("notes", "n1", { title: "z" });
    await db.documents.delete("notes", "n1");
    expect(calls.slice(2).map((c) => [c.method, c.url])).toEqual([
      ["PATCH", "http://db.test:3000/api/documents/notes/n1"],
      ["PUT", "http://db.test:3000/api/documents/notes/n1"],
      ["DELETE", "http://db.test:3000/api/documents/notes/n1"],
    ]);
  });

  test("a document field named collection is refused before any request", async () => {
    const { db, calls } = client(ok({}));
    await expect(db.documents.create("notes", { collection: "x" })).rejects.toBeInstanceOf(
      TypeError,
    );
    expect(calls).toHaveLength(0);
  });

  test("a missing document is null, not an error; list pages with query parameters", async () => {
    const { db, calls } = client({ status: 404, json: { error: "not found" } });
    expect(await db.documents.get("notes", "gone")).toBeNull();
    const listing = client(ok({ results: [{ _key: "a" }], count: 1 }));
    expect(
      (await listing.db.documents.list("my notes", { limit: 10, offset: 20 })) as unknown,
    ).toEqual([{ _key: "a" }]);
    expect(listing.calls[0]?.url).toBe(
      "http://db.test:3000/api/documents?collection=my+notes&limit=10&offset=20",
    );
    expect(calls).toHaveLength(1);
  });

  test("batch, indexes, health and the raw escape hatch", async () => {
    const { db, calls } = client(ok({ results: [{ _key: "a" }, null], count: 2 }));
    const results = await db.batch([
      { op: "insert", collection: "notes", doc: { _key: "a" } },
      { op: "delete", collection: "notes", key: "b" },
    ]);
    expect(results).toEqual([{ _key: "a" }, null]);
    expect(calls[0]?.body).toEqual({
      ops: [
        { op: "insert", collection: "notes", doc: { _key: "a" } },
        { op: "delete", collection: "notes", key: "b" },
      ],
    });
    await db.indexes.ensure("users", { fields: ["email"] });
    expect(calls[1]).toMatchObject({
      method: "POST",
      url: "http://db.test:3000/api/collections/users/indexes",
      body: { fields: ["email"] },
    });
    await db.indexes.list("users");
    await db.indexes.drop("users", "email/unique");
    expect(calls[3]?.url).toBe("http://db.test:3000/api/collections/users/indexes/email%2Funique");
    await db.health();
    expect(calls[4]?.url).toBe("http://db.test:3000/health");
    await db.request("GET", "/api/collections");
    expect(calls[5]?.url).toBe("http://db.test:3000/api/collections");
  });

  test("database health reports 503 instead of throwing", async () => {
    const { db } = client({ status: 503, json: { database: "disconnected", error: "ping" } });
    expect(await db.databaseHealth()).toEqual({
      ok: false,
      status: 503,
      body: { database: "disconnected", error: "ping" },
    });
  });

  test("login returns the token without changing the client; withToken derives one", async () => {
    const { fetch, calls } = client(ok({ token: "jwt", expires_in: 900 }), ok({ results: [] }));
    const anonymous = new CogniGraph({ baseUrl: BASE, fetch });
    const session = await anonymous.login("admin", "secret");
    expect(session).toEqual({ token: "jwt", expiresIn: 900 });
    expect(calls[0]?.headers.authorization).toBeUndefined();
    await anonymous.withToken(session.token).query("RETURN 1");
    expect(calls[1]?.headers.authorization).toBe("Bearer jwt");
    expect(anonymous.options.token).toBeUndefined();
  });

  test("custom headers are sent, and an empty base path segment is not doubled", async () => {
    const fake = fakeFetch(ok({ results: [] }));
    const db = new CogniGraph({
      baseUrl: "http://db.test/prefix//",
      fetch: fake.fetch,
      headers: { "x-tenant": "acme" },
    });
    await db.query("RETURN 1");
    expect(fake.calls[0]?.url).toBe("http://db.test/prefix/api/search/query");
    expect(fake.calls[0]?.headers["x-tenant"]).toBe("acme");
  });
});

describe("errors", () => {
  const cases: [
    number,
    Record<string, unknown> | string,
    new (...a: never[]) => CogniGraphError,
  ][] = [
    [400, { error: "syntax" }, BadRequestError],
    [403, { error: "no", code: "enterprise_feature_required" }, ForbiddenError],
    [404, { error: "gone" }, NotFoundError],
    [408, "request timed out", TimeoutError],
    [409, { error: "exists" }, ConflictError],
    [409, { error: "dup", code: "unique_violation" }, UniqueViolationError],
    [429, { error: "slow down" }, RateLimitError],
    [500, { error: "boom" }, ServerError],
    [503, { error: "down" }, UnavailableError],
  ];

  for (const [status, body, kind] of cases) {
    test(`${status} ${typeof body === "string" ? body : body.error} maps to ${kind.name}`, async () => {
      const reply = typeof body === "string" ? { status, text: body } : { status, json: body };
      const { db } = client(reply);
      const error = await db.mutate("RETURN 1").catch((e: unknown) => e);
      expect(error).toBeInstanceOf(kind);
      expect(error).toBeInstanceOf(CogniGraphError);
      const failure = error as CogniGraphError;
      expect(failure.status).toBe(status);
      expect(failure.message).toBe(typeof body === "string" ? body : String(body.error));
      expect(failure.code).toBe(typeof body === "string" ? undefined : (body.code as string));
      expect(failure.retryable).toBe([408, 429, 503].includes(status));
    });
  }

  test("a unique violation is also a conflict", async () => {
    const { db } = client({ status: 409, json: { error: "dup", code: "unique_violation" } });
    await expect(db.documents.create("users", { email: "a" })).rejects.toBeInstanceOf(
      ConflictError,
    );
  });

  test("Retry-After in seconds and as an HTTP date", async () => {
    const seconds = await rejection<RateLimitError>(
      client({ status: 429, json: {}, headers: { "retry-after": "2" } }).db.query("RETURN 1"),
    );
    expect(seconds.retryAfterMs).toBe(2000);
    const date = new Date(Date.now() + 5000).toUTCString();
    const dated = await rejection<UnavailableError>(
      client({ status: 503, json: {}, headers: { "retry-after": date } }).db.query("RETURN 1"),
    );
    expect(dated.retryAfterMs).toBeGreaterThan(3000);
    expect(dated.retryAfterMs).toBeLessThanOrEqual(5000);
    const junk = await rejection<UnavailableError>(
      client({ status: 503, json: {}, headers: { "retry-after": "soon" } }).db.query("RETURN 1"),
    );
    expect(junk.retryAfterMs).toBeUndefined();
  });

  test("network failures, bad success bodies and client timeouts", async () => {
    const network = await client(new TypeError("fetch failed"))
      .db.query("RETURN 1")
      .catch((e: unknown) => e);
    expect(network).toBeInstanceOf(NetworkError);
    expect((network as NetworkError).retryable).toBe(true);
    expect((network as NetworkError).status).toBe(0);

    const garbled = await client({ status: 200, text: "<html>" })
      .db.query("RETURN 1")
      .catch((e: unknown) => e);
    expect(garbled).toBeInstanceOf(ProtocolError);

    const fake = fakeFetch("hang");
    const slow = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, timeoutMs: 20 });
    const timeout = await slow.query("RETURN 1").catch((e: unknown) => e);
    expect(timeout).toBeInstanceOf(TimeoutError);
    expect((timeout as TimeoutError).status).toBe(0);
  });

  test("a caller's abort rejects with the caller's reason and is never retried", async () => {
    const fake = fakeFetch("hang");
    const db = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, retry: { attempts: 5 } });
    const controller = new AbortController();
    const pending = db.query("RETURN 1", {}, { signal: controller.signal });
    controller.abort(new Error("user navigated away"));
    await expect(pending).rejects.toThrow("user navigated away");
    expect(fake.calls).toHaveLength(1);
  });
});

describe("opt-in retries", () => {
  const fast = { attempts: 3, baseMs: 1, maxMs: 5 };

  test("off by default: one attempt even for a retryable read", async () => {
    const { db, calls } = client({ status: 503, json: { error: "down" } }, ok({ results: [] }));
    await expect(db.query("RETURN 1")).rejects.toBeInstanceOf(UnavailableError);
    expect(calls).toHaveLength(1);
  });

  test("reads retry 408, 429, 503 and network errors, then succeed", async () => {
    const fake = fakeFetch(
      { status: 503, json: {} },
      new TypeError("reset"),
      { status: 429, json: {}, headers: { "retry-after": "60" } },
      ok({ results: [7] }),
    );
    const db = new CogniGraph({
      baseUrl: BASE,
      fetch: fake.fetch,
      retry: { ...fast, attempts: 4 },
    });
    // Retry-After 60 s is capped by maxMs, so the test stays fast.
    expect(await db.query("RETURN 7")).toEqual([7]);
    expect(fake.calls).toHaveLength(4);
  });

  test("attempts are bounded and the last error is thrown", async () => {
    const fake = fakeFetch({ status: 503, json: { error: "down" } });
    const db = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, retry: fast });
    await expect(db.documents.list("notes")).rejects.toBeInstanceOf(UnavailableError);
    expect(fake.calls).toHaveLength(3);
  });

  test("errors that are not transient are never retried", async () => {
    for (const status of [400, 403, 404, 409, 500]) {
      const fake = fakeFetch({ status, json: { error: "x" } });
      const db = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, retry: fast });
      await db.query("RETURN 1").catch(() => undefined);
      expect(fake.calls).toHaveLength(1);
    }
  });

  test("mutations, batches, writes and login are never retried automatically", async () => {
    const fake = fakeFetch({ status: 503, json: { error: "down" } });
    const db = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, retry: fast });
    const writes: (() => Promise<unknown>)[] = [
      () => db.mutate("INSERT {} INTO notes"),
      () => db.batch([{ op: "delete", collection: "notes", key: "a" }]),
      () => db.documents.create("notes", {}),
      () => db.documents.update("notes", "a", {}),
      () => db.documents.replace("notes", "a", {}),
      () => db.documents.delete("notes", "a"),
      () => db.indexes.ensure("notes", { fields: ["a"] }),
      () => db.indexes.drop("notes", "a"),
      () => db.login("u", "p"),
    ];
    for (const write of writes) await write().catch(() => undefined);
    expect(fake.calls).toHaveLength(writes.length);
  });

  test("a per-call idempotent flag opts a raw request in", async () => {
    const fake = fakeFetch({ status: 503, json: {} }, ok({ fine: true }));
    const db = new CogniGraph({ baseUrl: BASE, fetch: fake.fetch, retry: fast });
    expect(await db.request<{ fine: boolean }>("PUT", "/api/x", {}, { idempotent: true })).toEqual({
      fine: true,
    });
    expect(fake.calls).toHaveLength(2);
  });

  test("invalid retry and timeout settings are refused at construction", () => {
    for (const options of [
      { retry: { attempts: 0 } },
      { retry: { attempts: 2.5 } },
      { retry: { attempts: 2, baseMs: -1 } },
      { timeoutMs: 0 },
      { timeoutMs: Number.NaN },
    ]) {
      expect(() => new CogniGraph({ baseUrl: BASE, ...options })).toThrow(RangeError);
    }
    expect(() => new CogniGraph({ baseUrl: "not a url" })).toThrow(TypeError);
  });
});
