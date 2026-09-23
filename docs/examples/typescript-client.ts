/**
 * Runnable example for the TypeScript client (clients/typescript).
 *
 *   COGNIGRAPH_URL=http://127.0.0.1:3000 COGNIGRAPH_PASSWORD=... bun docs/examples/typescript-client.ts
 *
 * Applications import from "@cognigraph/client"; this in-repo example imports
 * the source directly. It creates the `example_users` collection.
 */
import { CogniGraph, UniqueViolationError } from "../../clients/typescript/src/index.js";

const url = process.env.COGNIGRAPH_URL ?? "http://127.0.0.1:3000";
const password = process.env.COGNIGRAPH_PASSWORD;
if (!password) throw new Error("set COGNIGRAPH_PASSWORD to the admin password");

// Log in once; reads retry transient failures (408, 429, 503, network) up to three times.
const anonymous = new CogniGraph({ baseUrl: url, retry: { attempts: 3 } });
const db = anonymous.withToken((await anonymous.login("admin", password)).token);

// One unique constraint instead of a check-then-insert race in application code.
await db.indexes.ensure("example_users", { fields: ["email"] });
await db.documents.create("example_users", { _key: "ana", email: "ana@example.test", plan: "pro" });

try {
  await db.documents.create("example_users", { _key: "ana2", email: "ana@example.test" });
} catch (error) {
  if (!(error instanceof UniqueViolationError)) throw error;
  console.log(`constraint held: ${error.message}`);
}

// Parameterized reads, including the page window; never splice values into CGQL.
const page = await db.query<{ key: string; plan: string }>(
  "FOR u IN example_users FILTER u.plan == @plan SORT u._key LIMIT @offset, @count RETURN { key: u._key, plan: u.plan }",
  { plan: "pro", offset: 0, count: 20 },
);
console.log(page);

// Several writes that must succeed together.
await db.batch([
  { op: "update", collection: "example_users", key: "ana", merge: { plan: "team" } },
  { op: "insert", collection: "example_users", doc: { _key: "bo", email: "bo@example.test" } },
]);
console.log(await db.documents.get("example_users", "ana"));
