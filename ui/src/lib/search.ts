// Search-mode domain model for the Query screen. Shapes mirror
// crates/cognigraph-server/src/routes/search/*: every text mode requires a
// server-side embedding provider; hits come back as
// {document_id, score, document}.

import type { JsonObject } from "../types.ts";

export type SearchMode = "semantic" | "hybrid" | "vector" | "graph-augmented";

export interface SearchHit {
  document_id: string;
  score: number;
  document: JsonObject | null;
}

/// Only complete document handles have a navigable target. Encode each
/// identifier so query punctuation cannot change the destination or filter.
export function searchDocumentPath(id: string): string | undefined {
  const [collection, key, extra] = id.split("/");
  if (!collection || !key || extra !== undefined) return undefined;
  return `/collections/${encodeURIComponent(collection)}?doc=${encodeURIComponent(key)}`;
}

export const SEARCH_MODE_META: Record<
  SearchMode,
  { label: string; endpoint: string; hint: string }
> = {
  semantic: {
    label: "Semantic",
    endpoint: "/search/semantic",
    hint: "Your text is embedded server-side and matched against stored document vectors.",
  },
  hybrid: {
    label: "Hybrid",
    endpoint: "/search/hybrid",
    hint: "BM25 full-text and vector similarity, fused with reciprocal-rank fusion.",
  },
  vector: {
    label: "Vector",
    endpoint: "/search/vector",
    hint: "Bring your own pre-computed vector (JSON array) — no embedding call is made.",
  },
  "graph-augmented": {
    label: "Graph-augmented",
    endpoint: "/search/graph-augmented",
    hint: "Semantic seeds expanded through typed relationships; accepted rank-hint neurons reweight the results.",
  },
};

export interface SearchFormValues {
  query: string;
  collection: string;
  limit: number;
  threshold: number;
  /// vector mode only: JSON array text.
  vectorText: string;
  /// graph-augmented only.
  edgeCollection?: string;
  maxDepth: number;
}

/// Build the request body for a mode from the shared form values.
/// Throws with a user-facing message when vector JSON is malformed.
export function buildSearchRequest(mode: SearchMode, values: SearchFormValues): JsonObject {
  const base: JsonObject = {
    collection: values.collection,
    limit: values.limit,
    threshold: values.threshold,
  };
  switch (mode) {
    case "semantic":
      return { ...base, query: values.query };
    case "vector": {
      let vector: unknown;
      try {
        vector = JSON.parse(values.vectorText);
      } catch {
        throw new Error("The vector must be a JSON array of numbers.");
      }
      if (!Array.isArray(vector) || vector.some((v) => typeof v !== "number")) {
        throw new Error("The vector must be a JSON array of numbers.");
      }
      return { ...base, vector };
    }
    case "hybrid":
      // Native keeps text and vectors on the same documents, so one
      // collection choice feeds both leg targets.
      return {
        query: values.query,
        documents_collection: values.collection,
        embeddings_collection: values.collection,
        limit: values.limit,
        threshold: values.threshold,
      };
    case "graph-augmented":
      return {
        ...base,
        query: values.query,
        edge_collection: values.edgeCollection,
        max_depth: values.maxDepth,
      };
  }
}

/// Normalize a mode's response into a flat hit list (graph-augmented nests
/// its combined ranking under `results` too; unknown shapes yield []).
export function extractHits(response: JsonObject): SearchHit[] {
  const results = response.results;
  if (!Array.isArray(results)) return [];
  return results
    .filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null)
    .map((entry) => {
      const document =
        typeof entry.document === "object" && entry.document !== null
          ? (entry.document as JsonObject)
          : null;
      return {
        // Vector search omits the top-level id and only carries document._id.
        document_id: String(entry.document_id ?? entry.id ?? document?._id ?? ""),
        score: typeof entry.score === "number" ? entry.score : Number(entry.score ?? 0),
        document,
      };
    });
}

/// The embeddings-disabled server message, mapped to friendlier copy.
export function friendlySearchError(message: string): string {
  if (message.includes("No embedding provider")) {
    return "This server has no embedding provider configured (COGNIGRAPH_EMBEDDING_PROVIDER), so text search modes are unavailable. The Vector tab still works with pre-computed vectors.";
  }
  return message;
}
