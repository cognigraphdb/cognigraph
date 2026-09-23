// CG-84: the compiled package under plain Node, no bundler or TypeScript loader.
import assert from "node:assert/strict";
import { CogniGraph, NotFoundError, UniqueViolationError } from "../../dist/index.js";

const anonymous = new CogniGraph({ baseUrl: process.env.COGNIGRAPH_URL });
const { token } = await anonymous.login("admin", process.env.COGNIGRAPH_PASSWORD);
const db = anonymous.withToken(token);

assert.equal((await db.databaseHealth()).ok, true);
await db.indexes.ensure("cg84_node", { fields: ["slug"] });
await db.documents.create("cg84_node", { _key: "n1", slug: "same" });
await assert.rejects(
  db.documents.create("cg84_node", { _key: "n2", slug: "same" }),
  UniqueViolationError,
);
assert.deepEqual(await db.query("FOR d IN cg84_node RETURN d.slug"), ["same"]);
assert.equal(await db.documents.get("cg84_node", "absent"), null);
await assert.rejects(db.request("GET", "/api/no-such-route"), NotFoundError);
console.log("node smoke ok");
