/** CG-84: the client against a real server over real HTTP. */
import { afterAll, beforeAll, expect, test } from "bun:test";
import { join } from "node:path";
import {
  AuthenticationError,
  BadRequestError,
  CogniGraph,
  ConflictError,
  ForbiddenError,
  UniqueViolationError,
} from "../../src/index.js";
import { type LiveServer, PASSWORD, startServer } from "./server.js";

let server: LiveServer;
let db: CogniGraph;
const ROOT = join(import.meta.dir, "../..");

beforeAll(async () => {
  server = await startServer();
  const anonymous = new CogniGraph({ baseUrl: server.baseUrl });
  db = anonymous.withToken((await anonymous.login("admin", PASSWORD)).token);
});

afterAll(async () => {
  await server?.stop();
});

test("health, database readiness and identity", async () => {
  const health = await db.health();
  expect(["community", "enterprise"]).toContain(health.edition as string);
  expect(await db.databaseHealth()).toMatchObject({ ok: true, status: 200 });
});

test("bad credentials and a bad token are authentication errors", async () => {
  const anonymous = new CogniGraph({ baseUrl: server.baseUrl });
  await expect(anonymous.login("admin", "wrong")).rejects.toBeInstanceOf(AuthenticationError);
  await expect(anonymous.withToken("not-a-token").query("RETURN 1")).rejects.toBeInstanceOf(
    AuthenticationError,
  );
});

test("document CRUD with every key punctuation class, and a missing document is null", async () => {
  const key = "c_5(x)+,=;$!*'%:@.";
  const created = await db.documents.create("cg84_docs", { _key: key, title: "one", n: 1 });
  expect(created._key).toBe(key);
  const stored = await db.documents.get<{ title: string; n: number }>("cg84_docs", key);
  expect(stored).toMatchObject({ _key: key, _id: `cg84_docs/${key}`, title: "one", n: 1 });
  await db.documents.update("cg84_docs", key, { title: "two" });
  expect((await db.documents.get("cg84_docs", key))?.title).toBe("two");
  await db.documents.replace("cg84_docs", key, { only: true });
  const replaced = await db.documents.get("cg84_docs", key);
  expect(replaced?.only).toBe(true);
  expect(replaced?.title).toBeUndefined();
  expect((await db.documents.delete("cg84_docs", key)).deleted).toBe(true);
  expect(await db.documents.get("cg84_docs", key)).toBeNull();
});

test("list pages and CGQL LIMIT binds page the same rows", async () => {
  for (const n of [1, 2, 3, 4, 5]) await db.documents.create("cg84_pages", { _key: `p${n}`, n });
  const page = await db.documents.list("cg84_pages", { limit: 2, offset: 1 });
  expect(page).toHaveLength(2);
  const keys = await db.query<string>(
    "FOR d IN cg84_pages SORT d.n LIMIT @offset, @count RETURN d._key",
    { offset: 1, count: 2 },
  );
  expect(keys).toEqual(["p2", "p3"]);
});

test("unique constraints: declare, enforce on documents and CGQL, list and drop", async () => {
  await db.indexes.ensure("cg84_users", { fields: ["email"] });
  expect((await db.indexes.list("cg84_users")).map((i) => [i.fields, i.unique])).toEqual([
    [["email"], true],
  ]);
  await db.documents.create("cg84_users", { _key: "a", email: "a@example.test" });
  const duplicate = await db.documents
    .create("cg84_users", { _key: "b", email: "a@example.test" })
    .catch((e: unknown) => e);
  expect(duplicate).toBeInstanceOf(UniqueViolationError);
  expect(duplicate).toBeInstanceOf(ConflictError);
  expect((duplicate as UniqueViolationError).code).toBe("unique_violation");
  await expect(
    db.mutate("INSERT { _key: @k, email: @e } INTO cg84_users", { k: "c", e: "a@example.test" }),
  ).rejects.toBeInstanceOf(UniqueViolationError);
  const dropped = await db.indexes.drop("cg84_users", "email_unique");
  expect(dropped.dropped).toBe(true);
  await db.documents.create("cg84_users", { _key: "b", email: "a@example.test" });
});

test("mutations return rows and a failing batch applies nothing", async () => {
  const rows = await db.mutate<{ _key: string }>("INSERT { _key: @k } INTO cg84_batch RETURN NEW", {
    k: "kept",
  });
  expect(rows[0]?._key).toBe("kept");
  const failure = await db
    .batch([
      { op: "insert", collection: "cg84_batch", doc: { _key: "fresh" } },
      { op: "insert", collection: "cg84_batch", doc: { _key: "kept" } },
    ])
    .catch((e: unknown) => e);
  expect(failure).toBeInstanceOf(ConflictError);
  expect(await db.documents.get("cg84_batch", "fresh")).toBeNull();
  const applied = await db.batch([
    { op: "insert", collection: "cg84_batch", doc: { _key: "fresh" } },
    { op: "update", collection: "cg84_batch", key: "kept", merge: { touched: true } },
  ]);
  expect(applied).toHaveLength(2);
  expect((await db.documents.get("cg84_batch", "kept"))?.touched).toBe(true);
});

test("CGQL errors are typed: syntax is 400, a viewer's mutation is 403", async () => {
  await expect(db.query("FOR d IN")).rejects.toBeInstanceOf(BadRequestError);
  await db.request("POST", "/api/users", {
    username: "cg84viewer",
    password: PASSWORD,
    role: "viewer",
  });
  const anonymous = new CogniGraph({ baseUrl: server.baseUrl });
  const viewer = anonymous.withToken((await anonymous.login("cg84viewer", PASSWORD)).token);
  expect(await viewer.query("RETURN 1")).toEqual([1]);
  await expect(viewer.mutate("INSERT {} INTO cg84_docs")).rejects.toBeInstanceOf(ForbiddenError);
});

test("the compiled package runs under Node and the docs example runs under Bun", async () => {
  const env = { ...process.env, COGNIGRAPH_URL: server.baseUrl, COGNIGRAPH_PASSWORD: PASSWORD };
  const node = Bun.spawnSync(["node", join(ROOT, "test/live/node-smoke.mjs")], { env });
  expect(node.stderr.toString()).toBe("");
  expect(node.exitCode).toBe(0);
  expect(node.stdout.toString()).toContain("node smoke ok");
  const example = Bun.spawnSync(["bun", join(ROOT, "../../docs/examples/typescript-client.ts")], {
    env,
  });
  expect(example.stderr.toString()).toBe("");
  expect(example.exitCode).toBe(0);
  expect(example.stdout.toString()).toContain("constraint held");
});
