import { ApiError, type CogniGraphApi } from "../api/client.ts";
import type { JsonObject } from "../types.ts";
import { type ConstructChunk, chunkIdentity, parseChunks } from "./construct.ts";

type ImportApi = Pick<CogniGraphApi, "get" | "post">;
export interface ImportReview {
  space: string;
  chunks: ConstructChunk[];
  stored: (JsonObject | null)[];
}

export async function reviewImport(
  api: ImportApi,
  space: string,
  input: string,
): Promise<ImportReview> {
  if (!chunkIdentity(space)) throw new Error("Select a valid space first.");
  const chunks = await parseChunks(input);
  if (!chunks.length) throw new Error("Enter at least one chunk.");
  return { space, chunks, stored: await readStored(api, space, chunks) };
}

export async function submitImport(api: ImportApi, review: ImportReview): Promise<JsonObject> {
  // Re-read before mutation so changes made while the preview is open require
  // another review. The server's current ingest API has no compare-and-swap;
  // this is a UI stale-preview guard, not a transaction across other writers.
  const current = await readStored(api, review.space, review.chunks);
  if (JSON.stringify(current) !== JSON.stringify(review.stored)) {
    throw new Error("Stored chunks changed since this preview. Review the import again.");
  }
  return api.post<JsonObject>("/construct/ingest", {
    space_type: review.space,
    chunks: review.chunks,
  });
}

async function readStored(
  api: ImportApi,
  space: string,
  chunks: ConstructChunk[],
): Promise<(JsonObject | null)[]> {
  const stored: (JsonObject | null)[] = [];
  // Bound concurrent requests while checking every ID; no paginated scan or
  // result limit can silently classify an existing chunk as new.
  for (let offset = 0; offset < chunks.length; offset += 6) {
    stored.push(
      ...(await Promise.all(
        chunks.slice(offset, offset + 6).map(async (chunk) => {
          const key = `${chunkIdentity(space)}-${chunkIdentity(chunk.id)}`;
          let doc: JsonObject;
          try {
            doc = await api.get<JsonObject>(`/documents/chunks/${encodeURIComponent(key)}`);
          } catch (error) {
            if (error instanceof ApiError && error.status === 404) return null;
            throw error;
          }
          if (doc.space_id !== space || doc.chunk_id !== chunk.id) {
            throw new Error(
              `Chunk id ${chunk.id} collides with another stored identity. Choose a different id.`,
            );
          }
          return doc;
        }),
      )),
    );
  }
  return stored;
}
