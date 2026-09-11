import type { GraphDocument, JsonObject } from "../types.ts";

export interface DocumentEdit {
  key: string;
  patch: JsonObject;
}

const readOnly = new Set(["_id", "_key", "_rev", "created_at", "updated_at"]);

/** Send only changed top-level fields. PATCH preserves fields absent from reads
 * (including Native sidecar vectors). Never reconstruct a write from labels. */
export function editedDocument(document: GraphDocument, draft: string): DocumentEdit {
  const parsed: unknown = JSON.parse(draft);
  if (!isObject(parsed)) throw new Error("The document must be a JSON object.");
  const original = document.raw;
  for (const key of readOnly) {
    if (JSON.stringify(parsed[key]) !== JSON.stringify(original[key])) {
      throw new Error(`${key} is read-only. Restore its original value.`);
    }
  }
  assertNoRemovedFields(original, parsed);
  const patch = Object.fromEntries(
    Object.entries(parsed).filter(
      ([key, value]) =>
        !readOnly.has(key) && JSON.stringify(value) !== JSON.stringify(original[key]),
    ),
  );
  return { key: document._key, patch };
}

// Object deletions are not portable through the existing PATCH contract:
// Native replaces nested objects, while Arango merges them. Refuse a removal
// rather than pretending it succeeded. Arrays and scalar type changes are valid.
function assertNoRemovedFields(before: JsonObject, after: JsonObject, path = "") {
  for (const [key, value] of Object.entries(before)) {
    const name = path ? `${path}.${key}` : key;
    if (!Object.hasOwn(after, key)) {
      throw new Error(
        `Removing ${name} is not supported by this editor. Use null to clear its value.`,
      );
    }
    if (isObject(value) && isObject(after[key])) assertNoRemovedFields(value, after[key], name);
  }
}

function isObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
