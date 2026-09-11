// Run only against the disposable Native sidecar instance described in audit.md.
import { strict as assert } from "node:assert";
import { CogniGraphApi } from "../../src/api/client.ts";
import { normalizeDocument } from "../../src/lib/api-documents.ts";
import { editedDocument } from "../../src/lib/document-edit.ts";
import { documentJson } from "../../src/lib/documents.ts";
import type { JsonObject } from "../../src/types.ts";

const api = new CogniGraphApi({ baseUrl: "http://127.0.0.1:3002", token: "" });
await api.post("/documents", { collection: "qa_vector", _key: "one", title: "Before", content: "Keep raw text", embedding: [1, 0] });
const before = await api.get<JsonObject>("/documents/qa_vector/one");
assert.equal(before.embedding, undefined, "sidecar API read must omit the stored vector");
const query = { collection: "qa_vector", vector: [1, 0], limit: 5, threshold: 0.9 };
const vectorsBefore = await api.post<JsonObject>("/search/vector", query);
const doc = normalizeDocument(before, "qa_vector");
const draft = { ...JSON.parse(documentJson(doc)), title: "After" };
const edit = editedDocument(doc, JSON.stringify(draft));
assert.deepEqual(edit.patch, { title: "After" });
await api.patch(`/documents/qa_vector/${edit.key}`, edit.patch);
const after = await api.get<JsonObject>("/documents/qa_vector/one");
const vectorsAfter = await api.post<JsonObject>("/search/vector", query);
assert.equal(after.title, "After");
assert.equal(after.content, "Keep raw text");
for (const response of [vectorsBefore, vectorsAfter]) {
  const hits = response.results as JsonObject[];
  assert.equal(hits.length, 1, "sidecar vector must still be searchable after edit");
  assert.equal((hits[0]?.document as JsonObject)._key, "one");
}
await Bun.write(new URL("sidecar.json", import.meta.url), JSON.stringify({
  mode: "Native Community sidecar; UI helpers plus real HTTP, no browser",
  before, patch: edit.patch, after, vectorsBefore, vectorsAfter,
}, null, 2));
console.log("PASS: title edit preserves the hidden sidecar vector, verified by vector search before and after.");
