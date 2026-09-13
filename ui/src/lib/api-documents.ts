import type { GraphDocument, JsonObject } from "../types.ts";

export function normalizeDocument(value: JsonObject, collection = "documents"): GraphDocument {
  const key = String(value._key ?? crypto.randomUUID());
  // A stored vector IS the embedding state — an explicit status field only
  // refines it (e.g. "processing"); its absence must not read as "missing".
  const vector = Array.isArray(value.embedding) ? value.embedding : undefined;
  const embeddingValue = value.embedding_status ?? value.embeddingState;
  const embedding =
    embeddingValue === "ready" || embeddingValue === "processing"
      ? embeddingValue
      : vector && vector.length > 0
        ? "ready"
        : "missing";
  const known = new Set([
    "_id",
    "_key",
    "title",
    "category",
    "summary",
    "owner",
    "tags",
    "createdAt",
    "updatedAt",
    "embedding_status",
    "embeddingState",
    // Embedding infrastructure, not document content: keeping the vector
    // out of the remainder keeps the JSON tab readable and stops inspector
    // saves from copying it into a `content` object.
    "embedding",
    "embedding_text",
    "model",
    "dimensions",
    "content",
  ]);
  const remainder = Object.fromEntries(Object.entries(value).filter(([name]) => !known.has(name)));
  return {
    raw: structuredClone(value),
    _id: String(value._id ?? `${collection}/${key}`),
    _key: key,
    title: String(value.title ?? value.name ?? key),
    category: String(value.category ?? "uncategorized"),
    summary: String(value.summary ?? value.description ?? ""),
    owner: String(value.owner ?? "—"),
    tags: Array.isArray(value.tags) ? value.tags.map(String) : [],
    createdAt: String(value.createdAt ?? value.created_at ?? "—"),
    updatedAt: String(value.updatedAt ?? value.updated_at ?? "—"),
    embedding,
    model: typeof value.model === "string" ? value.model : undefined,
    dimensions:
      typeof value.dimensions === "number" ? value.dimensions : (vector?.length ?? undefined),
    content: isObject(value.content) ? value.content : remainder,
  };
}

/// The text an embedding is generated from: title, summary, and the
/// document's string content fields. Empty when the document has no
/// embeddable text.
export function embeddingSourceText(document: GraphDocument): string {
  const parts = [document.title, document.summary];
  if (typeof document.raw.content === "string") parts.push(document.raw.content);
  for (const value of Object.values(document.content)) {
    if (typeof value === "string") parts.push(value);
  }
  return parts
    .map((part) => part.trim())
    .filter(Boolean)
    .join("\n");
}

/// POST /documents/embed body that re-embeds one existing document: the
/// derived text is recorded under `embedding_text` (provenance of what was
/// embedded) rather than overwriting any document field, and upsert merges
/// into the existing document instead of conflicting.
export function embedDocumentRequest(collection: string, document: GraphDocument): JsonObject {
  return {
    collection,
    items: [
      {
        _key: document._key,
        embedding_text: embeddingSourceText(document),
        embedding_status: "ready",
      },
    ],
    text_field: "embedding_text",
    upsert: true,
  };
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
