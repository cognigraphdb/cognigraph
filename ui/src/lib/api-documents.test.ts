import { describe, expect, test } from "bun:test";
import {
  documentPayload,
  embedDocumentRequest,
  embeddingSourceText,
  normalizeDocument,
} from "./api-documents.ts";

describe("API document mapping", () => {
  test("normalizes a server document for the collection table", () => {
    const document = normalizeDocument({
      _id: "documents/alpha",
      _key: "alpha",
      title: "Alpha",
      embedding_status: "ready",
      tags: ["one", 2],
      extra: true,
    });

    expect(document.embedding).toBe("ready");
    expect(document.tags).toEqual(["one", "2"]);
    expect(document.content).toEqual({ extra: true });
  });

  test("creates a writable API payload without server identifiers", () => {
    const payload = documentPayload(
      normalizeDocument({ _id: "documents/alpha", _key: "alpha", title: "Alpha" }),
    );

    expect(payload.title).toBe("Alpha");
    expect(payload._id).toBeUndefined();
    expect(payload._key).toBeUndefined();
  });
});

describe("embedding state normalization", () => {
  test("a stored vector reads as ready even without a status field", () => {
    const doc = normalizeDocument(
      { _key: "a", title: "A", embedding: [0.1, 0.2, 0.3] },
      "label_index",
    );
    expect(doc.embedding).toBe("ready");
    expect(doc.dimensions).toBe(3);
  });

  test("an explicit processing status wins over the vector", () => {
    const doc = normalizeDocument(
      { _key: "a", embedding_status: "processing", embedding: [0.1] },
      "docs",
    );
    expect(doc.embedding).toBe("processing");
  });

  test("the vector and its provenance text stay out of content", () => {
    const doc = normalizeDocument(
      { _key: "a", embedding: [0.1], embedding_text: "A", note: "kept" },
      "docs",
    );
    expect(doc.content).toEqual({ note: "kept" });
  });
});

describe("embedDocumentRequest", () => {
  test("derives text from title, summary, and string content fields", () => {
    const doc = normalizeDocument(
      { _key: "a", title: "Aspirin", summary: "NSAID", text: "Take with water.", count: 3 },
      "labels",
    );
    expect(embeddingSourceText(doc)).toBe("Aspirin\nNSAID\nTake with water.");
    expect(embedDocumentRequest("labels", doc)).toEqual({
      collection: "labels",
      items: [
        {
          _key: "a",
          embedding_text: "Aspirin\nNSAID\nTake with water.",
          embedding_status: "ready",
        },
      ],
      text_field: "embedding_text",
      upsert: true,
    });
  });

  test("a document with no embeddable text yields an empty source", () => {
    // Whitespace title, no summary, and only non-string content fields.
    const doc = normalizeDocument({ _key: "a", title: "  ", count: 3 }, "docs");
    expect(embeddingSourceText(doc)).toBe("");
  });
});
