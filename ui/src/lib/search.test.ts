import { describe, expect, test } from "bun:test";
import type { SearchFormValues } from "./search.ts";
import {
  buildSearchRequest,
  extractHits,
  friendlySearchError,
  searchDocumentPath,
} from "./search.ts";

describe("searchDocumentPath", () => {
  test("encodes a document handle without introducing extra query parameters", () => {
    expect(searchDocumentPath("labels/rigel")).toBe("/collections/labels?doc=rigel");
    const path = searchDocumentPath("research notes/Rigel & Vega?#1");
    const url = new URL(path ?? "", "http://localhost");
    expect(url.pathname).toBe("/collections/research%20notes");
    expect([...url.searchParams]).toEqual([["doc", "Rigel & Vega?#1"]]);
    expect(url.hash).toBe("");
  });

  test("leaves incomplete or ambiguous handles without a navigation action", () => {
    for (const id of ["", "rigel", "/rigel", "labels/", "labels/rigel/extra", "labels/rigel/"]) {
      expect(searchDocumentPath(id)).toBeUndefined();
    }
  });
});

const values: SearchFormValues = {
  query: "corticosteroid cream",
  collection: "label_index",
  limit: 10,
  threshold: 0.35,
  vectorText: "[0.1, 0.2]",
  edgeCollection: "document_relations",
  maxDepth: 2,
};

describe("buildSearchRequest", () => {
  test("semantic sends query + collection + bounds", () => {
    expect(buildSearchRequest("semantic", values)).toEqual({
      collection: "label_index",
      limit: 10,
      threshold: 0.35,
      query: "corticosteroid cream",
    });
  });

  test("hybrid feeds both leg targets from the one collection choice", () => {
    const body = buildSearchRequest("hybrid", values);
    expect(body.documents_collection).toBe("label_index");
    expect(body.embeddings_collection).toBe("label_index");
    expect(body.query).toBe("corticosteroid cream");
  });

  test("vector parses the JSON array and rejects junk", () => {
    expect(buildSearchRequest("vector", values).vector).toEqual([0.1, 0.2]);
    expect(() => buildSearchRequest("vector", { ...values, vectorText: "nope" })).toThrow(
      /JSON array/,
    );
    expect(() => buildSearchRequest("vector", { ...values, vectorText: '["a"]' })).toThrow(
      /JSON array/,
    );
  });

  test("graph-augmented carries edge collection and depth", () => {
    const body = buildSearchRequest("graph-augmented", values);
    expect(body.edge_collection).toBe("document_relations");
    expect(body.max_depth).toBe(2);
  });
});

describe("extractHits", () => {
  test("maps the standard hit shape", () => {
    const hits = extractHits({
      results: [{ document_id: "labels/a", score: 0.91, document: { title: "A" } }],
      count: 1,
    });
    expect(hits).toEqual([{ document_id: "labels/a", score: 0.91, document: { title: "A" } }]);
  });

  test("falls back to document._id when the hit has no top-level id (vector mode)", () => {
    const hits = extractHits({
      results: [{ score: 0.99, document: { _id: "label_index/a", title: "A" } }],
    });
    expect(hits[0]?.document_id).toBe("label_index/a");
  });

  test("tolerates missing fields and non-array results", () => {
    expect(extractHits({})).toEqual([]);
    expect(extractHits({ results: [{}] })[0]).toEqual({
      document_id: "",
      score: 0,
      document: null,
    });
  });
});

describe("friendlySearchError", () => {
  test("maps the no-provider message to actionable copy", () => {
    expect(friendlySearchError("No embedding provider configured. Set ...")).toContain(
      "COGNIGRAPH_EMBEDDING_PROVIDER",
    );
  });

  test("passes other messages through", () => {
    expect(friendlySearchError("Collection not found: x")).toBe("Collection not found: x");
  });
});
