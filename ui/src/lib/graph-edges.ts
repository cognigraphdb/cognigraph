// Relationship creation from the Graph canvas. Shapes mirror
// crates/cognigraph-server/src/routes/graph.rs (POST /api/graph/relationships):
// the server upserts the edge and returns the stored document.

import type { JsonObject } from "../types.ts";

export interface RelationshipDraft {
  collection: string;
  from: string;
  to: string;
  relationType: string;
  confidence: number;
}

/// Validate a vertex address: `collection/key`, both parts non-empty,
/// no system (`_`-prefixed) collection. Returns a user-facing message,
/// or undefined when the id is well-formed.
export function vertexIdError(id: string): string | undefined {
  const trimmed = id.trim();
  if (!trimmed) return "Enter a vertex id.";
  const slash = trimmed.indexOf("/");
  if (slash <= 0 || slash === trimmed.length - 1) {
    return "Vertex ids look like collection/key.";
  }
  if (trimmed.startsWith("_")) return "System collections cannot be linked.";
  if (trimmed.slice(slash + 1).includes("/")) {
    return "Vertex ids have exactly one slash.";
  }
  return undefined;
}

/// Build the POST /graph/relationships body from a validated draft.
export function buildRelationshipRequest(draft: RelationshipDraft): JsonObject {
  return {
    collection: draft.collection,
    from: draft.from.trim(),
    to: draft.to.trim(),
    relation_type: draft.relationType.trim(),
    confidence: draft.confidence,
  };
}
