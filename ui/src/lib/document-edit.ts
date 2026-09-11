import type { GraphDocument } from "../types.ts";

export function editedDocument(document: GraphDocument, draft: string): GraphDocument {
  const parsed: unknown = JSON.parse(draft);
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("The document must be a JSON object.");
  }
  const fields = parsed as Record<string, unknown>;
  return {
    ...document,
    title: String(fields.title ?? document.title),
    category: String(fields.category ?? document.category),
    summary: String(fields.summary ?? document.summary),
    owner: String(fields.owner ?? document.owner),
    tags: Array.isArray(fields.tags) ? fields.tags.map(String) : document.tags,
    content:
      fields.content && typeof fields.content === "object"
        ? (fields.content as Record<string, unknown>)
        : document.content,
    updatedAt: new Date().toISOString(),
  };
}
