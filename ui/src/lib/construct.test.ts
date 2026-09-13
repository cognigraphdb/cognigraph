import { describe, expect, test } from "bun:test";
import { parseChunks, parseGaps, summarizeRun } from "./construct.ts";

describe("parseChunks", () => {
  test("distinct corpora have distinct identities, stable across retries and reordering", async () => {
    const first = await parseChunks("Aspirin treats pain.");
    const second = await parseChunks("Ibuprofen reduces fever.");
    expect(first[0]?.id).toMatch(/^ui-[a-f0-9]{64}$/);
    expect(first[0]?.id).not.toBe(second[0]?.id);
    expect(await parseChunks("  Aspirin treats pain.  \n")).toEqual(first);
    expect(await parseChunks("Ibuprofen reduces fever.\n\nAspirin treats pain.")).toEqual([
      ...second,
      ...first,
    ]);
  });

  test("JSONL preserves explicit ids and titles; absent ids use canonical content", async () => {
    const chunks = await parseChunks('{"id":"c1","title":"T","text":"alpha"}\n{"text":"beta"}');
    expect(chunks[0]).toEqual({ id: "c1", title: "T", text: "alpha" });
    expect(chunks[1]).toEqual((await parseChunks("beta"))[0]);
    expect(await parseChunks("café")).toEqual(await parseChunks("cafe\u0301"));
    expect((await parseChunks('{"title":"T","text":"beta"}'))[0]?.id).not.toBe(chunks[1]?.id);
  });

  test("rejects duplicate text, invalid ids and backend key collisions before writes", async () => {
    for (const input of ["same\nsame", '{"id":"A_b","text":"a"}\n{"id":"a-b","text":"b"}']) {
      await expect(parseChunks(input)).rejects.toThrow("duplicate or colliding");
    }
    for (const id of ["", "!!!", "K", 123, null]) {
      await expect(parseChunks(JSON.stringify({ id, text: "ok" }))).rejects.toThrow(
        "id must be a string",
      );
    }
    expect(await parseChunks('{"id":"a","text":"same"}\n{"id":"b","text":"same"}')).toHaveLength(2);
  });

  test("malformed JSONL names the line", async () => {
    await expect(parseChunks('{"text":"ok"}\n{nope')).rejects.toThrow("Line 2 is not valid JSON.");
    await expect(parseChunks('{"title":"no text"}')).rejects.toThrow(
      'Line 1 has no non-empty "text".',
    );
    await expect(parseChunks('{"title":12,"text":"ok"}')).rejects.toThrow("title must be a string");
  });

  test("empty input yields no chunks", async () => {
    expect(await parseChunks("  \n ")).toEqual([]);
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
