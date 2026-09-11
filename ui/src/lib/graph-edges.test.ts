import { describe, expect, test } from "bun:test";
import { buildRelationshipRequest, vertexIdError } from "./graph-edges.ts";

describe("vertexIdError", () => {
  test("accepts collection/key", () => {
    expect(vertexIdError("labels/abc-123")).toBeUndefined();
    expect(vertexIdError("  labels/abc  ")).toBeUndefined();
  });

  test("rejects malformed addresses", () => {
    expect(vertexIdError("")).toBe("Enter a vertex id.");
    expect(vertexIdError("labels")).toBe("Vertex ids look like collection/key.");
    expect(vertexIdError("/abc")).toBe("Vertex ids look like collection/key.");
    expect(vertexIdError("labels/")).toBe("Vertex ids look like collection/key.");
    expect(vertexIdError("a/b/c")).toBe("Vertex ids have exactly one slash.");
  });

  test("refuses system collections", () => {
    expect(vertexIdError("_users/admin")).toBe("System collections cannot be linked.");
  });
});

describe("buildRelationshipRequest", () => {
  test("maps the draft to the server contract and trims text", () => {
    expect(
      buildRelationshipRequest({
        collection: "label_relations",
        from: " labels/a ",
        to: "labels/b",
        relationType: " same_class ",
        confidence: 0.85,
      }),
    ).toEqual({
      collection: "label_relations",
      from: "labels/a",
      to: "labels/b",
      relation_type: "same_class",
      confidence: 0.85,
    });
  });
});
