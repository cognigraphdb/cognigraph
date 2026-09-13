import { describe, expect, it } from "bun:test";
import { normalizeDocument } from "./api-documents.ts";
import { editedDocument } from "./document-edit.ts";
import { documentJson } from "./documents.ts";

const raw = {
  _id: "qa/original",
  _key: "original",
  _rev: "revision",
  title: "Before",
  created_at: "2026-01-01",
  updated_at: "2026-01-02",
  tags: ["one", 2, null],
  custom: { enabled: false, count: 3, nested: [null, { label: "kept" }] },
  embedding: [0.1, 0.2],
};

describe("raw document editing", () => {
  for (const content of [{ note: "kept" }, "raw text", [1, null, { ok: true }], null, 42, false]) {
    it(`preserves ${JSON.stringify(content)} content during a title-only round trip`, () => {
      const stored = { ...raw, content };
      const document = normalizeDocument(stored, "qa");
      expect(JSON.parse(documentJson(document))).toEqual(stored);
      const draft = { ...JSON.parse(documentJson(document)), title: "After" };
      const edit = editedDocument(document, JSON.stringify(draft));
      expect(edit).toEqual({ key: "original", patch: { title: "After" } });
      // Model the Native top-level PATCH, including data absent from the API read.
      expect({ ...stored, private_sidecar: [0.3], ...edit.patch }).toEqual({
        ...stored,
        private_sidecar: [0.3],
        title: "After",
      });
      expect(document.raw).toEqual(stored);
    });
  }

  it("edits real custom fields and keeps their JSON types without template defaults", () => {
    const document = normalizeDocument(raw, "qa");
    const draft = { ...raw, custom: { ...raw.custom, count: null }, owner: false, new_field: [3] };
    const { patch } = editedDocument(document, JSON.stringify(draft));
    expect(patch).toEqual({ custom: draft.custom, owner: false, new_field: [3] });
    expect(patch.content).toBeUndefined();
    expect(patch.category).toBeUndefined();
    expect(patch.updatedAt).toBeUndefined();
  });

  it("does not resend unchanged fields, including when a read omits a sidecar vector", () => {
    const document = normalizeDocument({ _key: "original", text: "keep" });
    expect(editedDocument(document, documentJson(document)).patch).toEqual({});
    const edit = editedDocument(document, '{"_key":"original","text":"new"}');
    expect(edit.patch).toEqual({ text: "new" });
  });

  it("rejects changes to identity, revision, and server timestamps", () => {
    const document = normalizeDocument(raw);
    for (const key of ["_id", "_key", "_rev", "created_at", "updated_at"]) {
      expect(() => editedDocument(document, JSON.stringify({ ...raw, [key]: "changed" }))).toThrow(
        "read-only",
      );
    }
  });

  it("rejects field removal instead of silently keeping it on a merging backend", () => {
    const document = normalizeDocument(raw);
    const { title: _title, ...removed } = raw;
    expect(() => editedDocument(document, JSON.stringify(removed))).toThrow("Removing title");
    expect(() => editedDocument(document, JSON.stringify({ ...raw, custom: {} }))).toThrow(
      "Removing custom.enabled",
    );
    expect(editedDocument(document, JSON.stringify({ ...raw, custom: null })).patch).toEqual({
      custom: null,
    });
  });

  it("rejects invalid JSON and non-object documents before a write", () => {
    for (const draft of ["{bad", "null", "[]", "true", "1", '"text"']) {
      expect(() => editedDocument(normalizeDocument(raw), draft)).toThrow();
    }
  });
});
