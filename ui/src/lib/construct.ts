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
export function parseChunks(input: string): ConstructChunk[] {
  const lines = input
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) return [];
  if (lines[0]?.startsWith("{")) {
    return lines.map((line, index) => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(line);
      } catch {
        throw new Error(`Line ${index + 1} is not valid JSON.`);
      }
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw new Error(`Line ${index + 1} is not a JSON object.`);
      }
      const record = parsed as JsonObject;
      const text = typeof record.text === "string" ? record.text.trim() : "";
      if (!text) throw new Error(`Line ${index + 1} has no non-empty "text".`);
      return {
        id: String(record.id ?? `chunk-${index + 1}`),
        ...(typeof record.title === "string" && record.title ? { title: record.title } : {}),
        text,
      };
    });
  }
  return lines.map((text, index) => ({ id: `chunk-${index + 1}`, text }));
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
