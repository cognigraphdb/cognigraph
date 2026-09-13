import { describe, expect, test } from "bun:test";
import { ApiError, type CogniGraphApi } from "../api/client.ts";
import type { JsonObject } from "../types.ts";
import { reviewImport, submitImport } from "./construct-import.ts";

function backend(initial: JsonObject[] = []) {
  const rows = new Map(initial.map((row) => [`/documents/chunks/${row._key}`, row]));
  const writes: unknown[] = [];
  let failure: Error | undefined;
  const api: Pick<CogniGraphApi, "get" | "post"> = {
    async get<T>(path: string) {
      if (failure) throw failure;
      const row = rows.get(path);
      if (!row) throw new ApiError("Document not found", 404);
      return structuredClone(row) as T;
    },
    async post<T>(path: string, body?: unknown) {
      writes.push({ path, body });
      return { chunks: 1 } as T;
    },
  };
  return {
    api,
    rows,
    writes,
    fail: (error: Error) => {
      failure = error;
    },
  };
}

const stored = {
  _key: "demo-source",
  space_id: "demo",
  chunk_id: "source",
  text: "old",
  title: null,
};

describe("construction import review", () => {
  test("existing explicit ids retain current source text for replacement review", async () => {
    const state = backend([stored]);
    const review = await reviewImport(
      state.api,
      "demo",
      '{"id":"source","text":"new"}\n{"text":"Other source"}',
    );
    expect(review.stored).toEqual([stored, null]);
    expect(state.writes).toHaveLength(0);
    await submitImport(state.api, review);
    expect(state.writes).toEqual([
      { path: "/construct/ingest", body: { space_type: "demo", chunks: review.chunks } },
    ]);
  });

  test("changed or newly created chunks invalidate a stale preview without mutation", async () => {
    for (const initial of [[], [stored]]) {
      const state = backend(initial);
      const review = await reviewImport(state.api, "demo", '{"id":"source","text":"new"}');
      state.rows.set("/documents/chunks/demo-source", { ...stored, text: "another writer" });
      await expect(submitImport(state.api, review)).rejects.toThrow("changed since this preview");
      expect(state.writes).toHaveLength(0);
    }
  });

  test("denied reads, outages, and raw identity collisions fail closed", async () => {
    for (const error of [
      new ApiError("Forbidden", 403),
      new ApiError("Outage", 500),
      new Error("Network"),
    ]) {
      const state = backend();
      state.fail(error);
      await expect(reviewImport(state.api, "demo", "text")).rejects.toThrow(error.message);
      expect(state.writes).toHaveLength(0);
    }
    const state = backend([{ ...stored, chunk_id: "SOURCE" }]);
    await expect(reviewImport(state.api, "demo", '{"id":"source","text":"new"}')).rejects.toThrow(
      "collides",
    );
    expect(state.writes).toHaveLength(0);
  });

  test("checks every chunk beyond a small page and refuses empty input", async () => {
    const state = backend();
    const review = await reviewImport(
      state.api,
      "demo",
      Array.from({ length: 101 }, (_, i) => `Text ${i}`).join("\n"),
    );
    expect(review.chunks).toHaveLength(101);
    expect(review.stored).toHaveLength(101);
    await expect(reviewImport(state.api, "demo", " \n")).rejects.toThrow("at least one chunk");
  });
});
