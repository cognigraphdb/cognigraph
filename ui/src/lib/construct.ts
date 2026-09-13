// Construct pipeline domain model. Shapes mirror
// crates/cognigraph-server/src/routes/construct.rs: ingest and advise are
// deterministic; draft/propose/review need the server's completion
// provider (their 4xx says so); evaluate needs a stored or inline eval
// spec. Chunks are `{id, title?, text}`.

import type { JsonObject } from "../types.ts";

export interface ConstructChunk {
  id: string;
  title?: string;
  text: string;
}

/// Parse a corpus textarea into chunks. Two formats:
/// - JSONL: every non-empty line is a `{id?, title?, text}` object
///   (detected by a leading `{`); a malformed line throws with its number.
/// - Plain text: every non-empty line becomes one chunk, ids generated.
export async function parseChunks(input: string): Promise<ConstructChunk[]> {
  const lines = input
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const jsonl = lines[0]?.startsWith("{");
  const chunks: ConstructChunk[] = [];
  const identities = new Set<string>();
  for (const [index, line] of lines.entries()) {
    let record: JsonObject = { text: line };
    if (jsonl) {
      let parsed: unknown;
      try {
        parsed = JSON.parse(line);
      } catch {
        throw new Error(`Line ${index + 1} is not valid JSON.`);
      }
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw new Error(`Line ${index + 1} is not a JSON object.`);
      }
      record = parsed as JsonObject;
    }
    const text = typeof record.text === "string" ? record.text.trim().normalize("NFC") : "";
    if (!text) throw new Error(`Line ${index + 1} has no non-empty "text".`);
    if (record.title !== undefined && typeof record.title !== "string") {
      throw new Error(`Line ${index + 1}: title must be a string.`);
    }
    const title = typeof record.title === "string" ? record.title : undefined;
    if (record.id !== undefined && (typeof record.id !== "string" || !chunkIdentity(record.id))) {
      throw new Error(
        `Line ${index + 1}: id must be a string containing an ASCII letter or digit.`,
      );
    }
    // Versioned content identity survives retries, reordering, and browser reloads.
    // Titles are part of source identity; missing and empty titles stay distinct.
    const id = typeof record.id === "string" ? record.id : await generatedChunkId(text, title);
    const identity = chunkIdentity(id);
    if (identities.has(identity)) {
      throw new Error(
        `Line ${index + 1}: duplicate or colliding chunk id ${id}. Use distinct JSONL ids for repeated text.`,
      );
    }
    identities.add(identity);
    chunks.push({ id, ...(title === undefined ? {} : { title }), text });
  }
  return chunks;
}

async function generatedChunkId(text: string, title?: string): Promise<string> {
  const bytes = new TextEncoder().encode(
    JSON.stringify(["cognigraph-ui-chunk-v1", title ?? null, text]),
  );
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return `ui-${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

/** Mirrors cognigraph-construct ingest::sanitize for direct chunk lookup.
 * The server still validates the stored raw identities and rejects collisions. */
export function chunkIdentity(id: string): string {
  return id
    .replace(/[^A-Za-z0-9]+/g, "-")
    .toLowerCase()
    .replace(/^-|-$/g, "");
}

/// Parse the propose gaps textarea: one `A --REL--> B` line each.
export function parseGaps(input: string): string[] {
  const lines = input
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  for (const [index, line] of lines.entries()) {
    if (!/--.+-->/.test(line)) {
      throw new Error(`Line ${index + 1} is not \`A --REL--> B\`.`);
    }
  }
  return lines;
}

export type ConstructAction =
  | "ingest"
  | "draft"
  | "accept"
  | "evaluate"
  | "propose"
  | "review"
  | "advise";

/// One line for the toast: the field that tells whether the run did
/// anything, per action.
export function summarizeRun(action: ConstructAction, response: JsonObject): string {
  switch (action) {
    case "ingest":
      return `Grounded ${response.facts_grounded ?? 0} facts from ${response.chunks ?? 0} chunks`;
    case "draft":
      return `Draft stored: ${response.entities ?? 0} entities, ${response.relation_rules ?? 0} rules (inert until accepted)`;
    case "accept":
      return `Space ${response.space_type ?? ""} accepted: ${response.relation_rules ?? 0} rules live`;
    case "evaluate": {
      const recall = response.recall as JsonObject | undefined;
      const restraint = response.restraint as JsonObject | undefined;
      return `Recall ${recall?.found ?? 0}/${recall?.total ?? 0}, restraint violations ${
        restraint?.violations ?? 0
      }/${restraint?.total ?? 0}`;
    }
    case "propose": {
      const proposed = Array.isArray(response.proposed) ? response.proposed.length : 0;
      const skipped = Array.isArray(response.skipped) ? response.skipped.length : 0;
      return `Proposed ${proposed} neurons (${skipped} skipped) — review them on the Review page`;
    }
    case "review":
      return `Reviewed ${response.reviewed ?? 0}: ${
        Array.isArray(response.auto_accepted) ? response.auto_accepted.length : 0
      } auto-accepted, ${Array.isArray(response.queued) ? response.queued.length : 0} queued for a human`;
    case "advise": {
      const suggestions = Array.isArray(response.suggestions)
        ? response.suggestions.length
        : (response.suggestions ?? 0);
      const flags = Array.isArray(response.review_flags)
        ? response.review_flags.length
        : (response.review_flags ?? 0);
      return `Advisor: ${suggestions} safe gate suggestions, ${flags} review flags`;
    }
  }
}
