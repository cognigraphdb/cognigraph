import { describe, expect, test } from "bun:test";
import { mergeExplorerGraphs, parseTraversalResponse, pathForSelection } from "./graph-explorer.ts";

const traversal = {
  count: 1,
  results: [
    {
      depth: 2,
      score: 0.72,
      vertices: [
        { _id: "documents/a", _key: "a", title: "Start" },
        { _id: "documents/b", _key: "b", title: "Middle" },
        { _id: "documents/c", _key: "c", title: "Target" },
      ],
      edges: [
        {
          _id: "relations/ab",
          _from: "documents/a",
          _to: "documents/b",
          relation_type: "supports",
          confidence: 0.9,
        },
        {
          _id: "relations/bc",
          _from: "documents/b",
          _to: "documents/c",
          relation_type: "depends_on",
          confidence: 0.8,
        },
      ],
    },
  ],
};

describe("graph explorer data", () => {
  test("maps traversal paths into deduplicated nodes and edges", () => {
    const graph = parseTraversalResponse(traversal);
    expect(graph.nodes).toHaveLength(3);
    expect(graph.edges).toHaveLength(2);
    expect(graph.paths[0]).toMatchObject({ depth: 2, confidence: 0.8, score: 0.72 });
    expect(pathForSelection(graph, undefined, "relations/bc")?.vertexIds).toEqual([
      "documents/a",
      "documents/b",
      "documents/c",
    ]);
  });

  test("merges an expanded neighborhood without duplicating the anchor", () => {
    const initial = parseTraversalResponse(traversal);
    const expanded = parseTraversalResponse({
      results: [
        {
          depth: 1,
          score: 0.8,
          vertices: [
            { _id: "documents/c", _key: "c" },
            { _id: "documents/d", _key: "d" },
          ],
          edges: [
            {
              _from: "documents/c",
              _to: "documents/d",
              relation_type: "cites",
              confidence: 1,
            },
          ],
        },
      ],
    });
    const merged = mergeExplorerGraphs(initial, expanded, "documents/c");
    expect(merged.nodes).toHaveLength(4);
    expect(merged.nodes.find((node) => node.id === "documents/d")?.depth).toBe(3);
  });
});
