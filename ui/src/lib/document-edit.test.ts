import { describe, expect, it } from "bun:test";
import { normalizeDocument } from "./api-documents.ts";
import { editedDocument } from "./document-edit.ts";

const original = normalizeDocument(
  {
    _key: "original",
    title: "Before",
    category: "research",
    updatedAt: "2026-01-01T00:00:00.000Z",
  },
  "qa",
);

describe("document edits", () => {
  it("preserves identity and omitted fields while using the current edit time", () => {
    const before = Date.now();
    const edited = editedDocument(original, '{"_key":"other","title":"After"}');
    expect(edited._key).toBe("original");
    expect(edited._id).toBe(original._id);
    expect(edited.category).toBe("research");
    expect(edited.title).toBe("After");
    expect(Date.parse(edited.updatedAt)).toBeGreaterThanOrEqual(before);
    expect(Date.parse(edited.updatedAt)).toBeLessThanOrEqual(Date.now());
    expect(original.title).toBe("Before");
  });

  it("rejects invalid JSON and non-object documents before a write", () => {
    for (const draft of ["{bad", "null", "[]", "true", "1", '"text"']) {
      expect(() => editedDocument(original, draft)).toThrow();
    }
  });
});
