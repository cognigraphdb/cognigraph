import { describe, expect, test } from "bun:test";
import { initialDocuments } from "../data/documents.ts";
import { filterDocuments } from "./documents.ts";

describe("filterDocuments", () => {
  test("combines search, category, and embedding filters", () => {
    const result = filterDocuments(initialDocuments, {
      search: "product",
      category: "strategy",
      embedding: "ready",
    });

    expect(result.map((document) => document.title)).toEqual(["Q2 2026 Product Strategy"]);
  });

  test("finds documents by key", () => {
    const result = filterDocuments(initialDocuments, {
      search: "91b2d6e7",
      category: "all",
      embedding: "all",
    });

    expect(result).toHaveLength(1);
    expect(result[0]?.category).toBe("postmortem");
  });
});
