import { describe, expect, test } from "bun:test";
import { parseChunks, parseGaps, summarizeRun } from "./construct.ts";

describe("parseChunks", () => {
  test("plain text lines become chunks with generated ids", () => {
    expect(parseChunks("Aspirin treats pain.\n\n  Ibuprofen reduces fever.  \n")).toEqual([
      { id: "chunk-1", text: "Aspirin treats pain." },
      { id: "chunk-2", text: "Ibuprofen reduces fever." },
    ]);
  });

  test("JSONL lines keep ids and titles", () => {
    expect(parseChunks('{"id": "c1", "title": "T", "text": "alpha"}\n{"text": "beta"}')).toEqual([
      { id: "c1", title: "T", text: "alpha" },
      { id: "chunk-2", text: "beta" },
    ]);
  });

  test("malformed JSONL names the line", () => {
    expect(() => parseChunks('{"text": "ok"}\n{nope')).toThrow("Line 2 is not valid JSON.");
    expect(() => parseChunks('{"title": "no text"}')).toThrow('Line 1 has no non-empty "text".');
  });

  test("empty input yields no chunks", () => {
    expect(parseChunks("  \n ")).toEqual([]);
  });
});

describe("parseGaps", () => {
  test("accepts fact lines and rejects junk", () => {
    expect(parseGaps("aspirin --TREATS--> pain\n")).toEqual(["aspirin --TREATS--> pain"]);
    expect(() => parseGaps("not a fact")).toThrow("Line 1 is not `A --REL--> B`.");
  });
});

describe("summarizeRun", () => {
  test("summarizes each action from its response fields", () => {
    expect(summarizeRun("ingest", { facts_grounded: 4, chunks: 3 })).toBe(
      "Grounded 4 facts from 3 chunks",
    );
    expect(summarizeRun("draft", { entities: 5, relation_rules: 2 })).toContain("5 entities");
    expect(summarizeRun("propose", { proposed: [{}, {}], skipped: [] })).toContain(
      "Proposed 2 neurons",
    );
    expect(summarizeRun("review", { reviewed: 3, auto_accepted: [{}], queued: [{}, {}] })).toBe(
      "Reviewed 3: 1 auto-accepted, 2 queued for a human",
    );
    expect(summarizeRun("advise", { suggestions: [{}], review_flags: [] })).toBe(
      "Advisor: 1 safe gate suggestions, 0 review flags",
    );
  });
});

describe("summarizeRun evaluate", () => {
  test("reads recall and restraint counters", () => {
    expect(
      summarizeRun("evaluate", {
        recall: { found: 3, total: 4 },
        restraint: { violations: 0, total: 2 },
      }),
    ).toBe("Recall 3/4, restraint violations 0/2");
  });
});
