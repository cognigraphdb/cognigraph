import { afterEach, describe, expect, test } from "bun:test";
import { CogniGraphApi } from "../api/client.ts";
import { normalizeDocument } from "./api-documents.ts";
import { collectionView, loadCollectionSearch } from "./collection-search.ts";

const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = originalFetch;
});
const api = new CogniGraphApi({ baseUrl: "http://synthetic.invalid", token: "" });
const docs = Array.from({ length: 125 }, (_, i) =>
  normalizeDocument(
    {
      _key: `item-${i}`,
      title: `Constellation ${i}`,
      category: i < 25 ? "first-page" : "later-only",
      embedding_status: i < 25 ? "missing" : "ready",
    },
    "qa",
  ),
);
const all = { category: "all", embedding: "all" };
const paging = { page: 1, pageSize: 25, search: false, total: 125 };

describe("bounded collection search", () => {
  test("125 available text matches remain a labelled cap with an additional exact key", async () => {
    const requests: string[] = [];
    globalThis.fetch = (async (url, init) => {
      const path = new URL(String(url)).pathname;
      requests.push(path);
      if (path === "/api/search/text") {
        const body = JSON.parse(String(init?.body));
        expect(body).toEqual({
          collection: "qa",
          query: "constellation",
          fields: ["title", "content", "summary", "text"],
          limit: 100,
        });
        return Response.json({
          results: docs.slice(0, body.limit).map((doc) => ({ document: doc.raw })),
          count: 100,
        });
      }
      return Response.json({ _key: "constellation", title: "Exact key" });
    }) as typeof fetch;
    const result = await loadCollectionSearch(api, "qa", "constellation");
    expect(requests.sort()).toEqual(["/api/documents/qa/constellation", "/api/search/text"]);
    expect(result.documents).toHaveLength(101);
    expect(result.documents[0]?._key).toBe("constellation");
    expect(result.textLimitReached).toBe(true);
    expect(result.errors).toEqual([]);
    const view = collectionView(result.documents, all, { ...paging, search: true });
    expect(view.summary).toBe("1–25 of 101 retrieved");
    expect(view.hasNext).toBe(true);
    expect(
      collectionView(result.documents, all, { ...paging, search: true, page: 5 }).summary,
    ).toBe("101–101 of 101 retrieved");
  });

  test("an exact key within text results is deduplicated and keeps its direct read", async () => {
    globalThis.fetch = (async (url) =>
      Response.json(
        String(url).endsWith("/search/text")
          ? { results: [{ document: { _key: "item-0", title: "Search copy" } }] }
          : { _key: "item-0", title: "Direct read" },
      )) as typeof fetch;
    const result = await loadCollectionSearch(api, "qa", "item-0");
    expect(result.documents.map((doc) => doc.title)).toEqual(["Direct read"]);
    expect(result.textLimitReached).toBe(false);
  });

  test("only a key 404 is a normal absence; failed text search remains explicit", async () => {
    globalThis.fetch = (async (url) =>
      String(url).endsWith("/search/text")
        ? Response.json({ error: "Unavailable" }, { status: 503 })
        : Response.json({ error: "Not found" }, { status: 404 })) as typeof fetch;
    const result = await loadCollectionSearch(api, "qa", "missing");
    expect(result.documents).toEqual([]);
    expect(result.errors).toEqual(["Text search failed: Unavailable"]);
  });

  test("denied exact-key lookup does not hide successful text hits or the failure", async () => {
    globalThis.fetch = (async (url) =>
      String(url).endsWith("/search/text")
        ? Response.json({ results: [{ document: docs[0]?.raw }] })
        : Response.json({ error: "Forbidden" }, { status: 403 })) as typeof fetch;
    const result = await loadCollectionSearch(api, "qa", "term");
    expect(result.documents).toHaveLength(1);
    expect(result.errors).toEqual(["Exact-key lookup failed: Forbidden"]);
  });

  test("exact key survives failed text search, and a retry can replace partial results", async () => {
    let fail = true;
    globalThis.fetch = (async (url) =>
      String(url).endsWith("/search/text")
        ? fail
          ? Response.json({ error: "Unavailable" }, { status: 503 })
          : Response.json({ results: [{ document: docs[0]?.raw }] })
        : Response.json({ _key: "exact" })) as typeof fetch;
    const partial = await loadCollectionSearch(api, "qa", "exact");
    expect(partial.documents.map((doc) => doc._key)).toEqual(["exact"]);
    expect(partial.errors).toHaveLength(1);
    fail = false;
    const recovered = await loadCollectionSearch(api, "qa", "exact");
    expect(recovered.documents).toHaveLength(2);
    expect(recovered.errors).toEqual([]);
  });
});

describe("page-local filters and retrieved-result pagination", () => {
  test("a filtered-empty first page still advances to later embedding states", () => {
    const ready = { ...all, embedding: "ready" };
    const first = collectionView(docs.slice(0, 25), ready, paging);
    expect(first.visible).toEqual([]);
    expect(first.categories).toEqual(["first-page"]);
    expect(first.summary).toBe("0 of 25 on this page");
    expect(first.hasNext).toBe(true);
    const second = collectionView(docs.slice(25, 50), ready, { ...paging, page: 2 });
    expect(second.visible).toHaveLength(25);
    expect(second.categories).toEqual(["later-only"]);
    expect(second.summary).toBe("25 of 25 on this page");
  });

  test("without a catalog total, next-page availability uses unfiltered rows", () => {
    const view = collectionView(
      docs.slice(0, 25),
      { ...all, category: "later-only" },
      { ...paging, total: undefined },
    );
    expect(view.hasNext).toBe(true);
    expect(view.summary).toBe("0 of 25 on this page");
    expect(view.categories).toEqual(["first-page", "later-only"]);
  });

  test("retrieved filters apply before paging and clamp a now-out-of-range page", () => {
    const view = collectionView(
      docs.slice(0, 100),
      { ...all, category: "first-page" },
      { ...paging, search: true, page: 4 },
    );
    expect(view.page).toBe(1);
    expect(view.visible).toHaveLength(25);
    expect(view.summary).toBe("1–25 of 25 filtered retrieved");
    expect(view.hasNext).toBe(false);
  });

  test("reset restores full page counts; a terminal page cannot advance", () => {
    const first = collectionView(docs.slice(0, 25), all, paging);
    expect(first.summary).toBe("1–25 of 125 documents");
    const last = collectionView(docs.slice(100), all, { ...paging, page: 5 });
    expect(last.summary).toBe("101–125 of 125 documents");
    expect(last.hasNext).toBe(false);
  });

  test("zero retrieved matches stays distinct from the collection total", () => {
    const empty = collectionView([], all, { ...paging, search: true });
    expect(empty.summary).toBe("0 of 0 retrieved");
    expect(empty.hasNext).toBe(false);
  });
});
