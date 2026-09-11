import { afterEach, describe, expect, test } from "bun:test";
import { CogniGraphApi } from "../api/client.ts";
import type { NeuronDoc } from "./neurons.ts";
import {
  loadNeuronPage,
  loadSelectedNeuron,
  loadSpaceTypes,
  neuronPageLabel,
  reviewLocation,
} from "./review-data.ts";

const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = originalFetch;
});
const api = new CogniGraphApi({ baseUrl: "http://synthetic.invalid", token: "" });
const signal = () => new AbortController().signal;
function serve(handler: (url: URL, init?: RequestInit) => unknown) {
  globalThis.fetch = (async (url, init) =>
    Response.json(handler(new URL(String(url)), init))) as typeof fetch;
}
const rows = (count: number) =>
  Array.from(
    { length: count },
    (_, i) => ({ _key: `neuron-${i}`, id: `neuron-${i}`, space_type: "qa" }) as NeuronDoc,
  );

describe("complete space catalogs", () => {
  test("continues beyond 100 spaces and ignores the response's page count", async () => {
    const offsets: number[] = [];
    const spaces = Array.from({ length: 201 }, (_, i) => ({
      _key: `space-${String(i).padStart(3, "0")}`,
    }));
    serve((url) => {
      const offset = Number(url.searchParams.get("offset"));
      offsets.push(offset);
      expect(url.searchParams.get("limit")).toBe("100");
      return { results: spaces.slice(offset, offset + 100), count: 100 };
    });
    expect(await loadSpaceTypes(api, signal())).toEqual(spaces.map((s) => s._key));
    expect(offsets).toEqual([0, 100, 200]);
  });
  test("an exact page boundary needs an empty continuation before completion", async () => {
    const offsets: string[] = [];
    serve((url) => {
      offsets.push(url.searchParams.get("offset") ?? "");
      return {
        results:
          offsets.length === 1 ? Array.from({ length: 100 }, (_, i) => ({ _key: `s-${i}` })) : [],
      };
    });
    expect((await loadSpaceTypes(api, signal())).length).toBe(100);
    expect(offsets).toEqual(["0", "100"]);
  });
  test("a failed second page rejects the catalog instead of returning the first 100", async () => {
    globalThis.fetch = (async (url) =>
      new URL(String(url)).searchParams.get("offset") === "0"
        ? Response.json({ results: Array.from({ length: 100 }, (_, i) => ({ _key: `s-${i}` })) })
        : Response.json({ error: "Unavailable continuation" }, { status: 503 })) as typeof fetch;
    await expect(loadSpaceTypes(api, signal())).rejects.toThrow("Unavailable continuation");
  });
  test("repeated pages fail explicitly instead of looping or inventing completeness", async () => {
    serve(() => ({ results: Array.from({ length: 100 }, (_, i) => ({ _key: `s-${i}` })) }));
    await expect(loadSpaceTypes(api, signal())).rejects.toThrow("catalog changed");
  });
  test("cancellation stops continuation and passes the signal to the request", async () => {
    const controller = new AbortController();
    let calls = 0;
    serve((_url, init) => {
      expect(init?.signal).toBe(controller.signal);
      calls += 1;
      controller.abort();
      return { results: Array.from({ length: 100 }, (_, i) => ({ _key: `s-${i}` })) };
    });
    await expect(loadSpaceTypes(api, controller.signal)).rejects.toThrow();
    expect(calls).toBe(1);
  });
});

describe("review continuation", () => {
  test("all 201 neurons are reachable once through bounded pages and truthful labels", async () => {
    const neurons = rows(201);
    const seen: string[] = [];
    serve((url) => {
      expect(url.searchParams.get("space_type")).toBe("qa");
      expect(url.searchParams.get("status")).toBe("proposed");
      expect(url.searchParams.get("limit")).toBe("51");
      const offset = Number(url.searchParams.get("offset"));
      return { neurons: neurons.slice(offset, offset + 51), count: 99999 };
    });
    for (let page = 1; page <= 5; page++) {
      const result = await loadNeuronPage(api, "qa", "proposed", page, signal());
      seen.push(...result.neurons.map((n) => n._key));
      expect(result.hasMore).toBe(page < 5);
      expect(neuronPageLabel(result)).toContain(
        page < 5 ? "More available" : "201–201 shown · End of queue",
      );
      expect(neuronPageLabel(result)).not.toContain("99999");
    }
    expect(seen).toEqual(neurons.map((n) => n._key));
  });
  test("exactly 50 rows are a terminal page; All omits the status predicate", async () => {
    serve((url) => {
      expect(url.searchParams.has("status")).toBe(false);
      return { neurons: rows(50), count: 50 };
    });
    const result = await loadNeuronPage(api, "qa", "all", 1, signal());
    expect(result.hasMore).toBe(false);
    expect(neuronPageLabel(result)).toBe("1–50 shown · End of queue");
  });
  test("a removed final row or stale far-away page recovers with one first-page retry", async () => {
    const offsets: number[] = [];
    serve((url) => {
      const offset = Number(url.searchParams.get("offset"));
      offsets.push(offset);
      return { neurons: offset ? [] : rows(51), count: 0 };
    });
    const result = await loadNeuronPage(api, "qa", "proposed", 50000, signal());
    expect(offsets).toEqual([2499950, 0]);
    expect(result.page).toBe(1);
    expect(result.hasMore).toBe(true);
  });
  test("empty and malformed responses stay distinguishable", async () => {
    serve(() => ({ neurons: [] }));
    expect(neuronPageLabel(await loadNeuronPage(api, "qa", "all", 1, signal()))).toBe("0 neurons");
    serve(() => ({ count: 0 }));
    await expect(loadNeuronPage(api, "qa", "all", 1, signal())).rejects.toThrow("Invalid neuron");
  });
});

describe("selection independent of the queue page", () => {
  test("direct lookup resolves an off-page item with its current persisted status", async () => {
    serve((url) => {
      expect(url.pathname).toBe("/api/documents/neurons/qa-accepted-200");
      return { ...rows(1)[0], _key: "qa-accepted-200", status: "retired" };
    });
    expect((await loadSelectedNeuron(api, "qa", "qa-accepted-200", signal())).status).toBe(
      "retired",
    );
  });
  test("a stale or wrong-space selection cannot populate the inspector", async () => {
    serve(() => ({ _key: "selected", space_type: "different" }));
    await expect(loadSelectedNeuron(api, "qa", "selected", signal())).rejects.toThrow(
      "does not belong",
    );
  });
  test("URL filters and selected identity survive a round trip; invalid pages are bounded", () => {
    expect(
      reviewLocation(new URLSearchParams("space=qa&status=accepted&page=5&neuron=last")),
    ).toEqual({ space: "qa", status: "accepted", page: 5, selectedKey: "last" });
    for (const page of ["-1", "0", "1.5", "NaN", "Infinity", "99999999999999999999"]) {
      expect(reviewLocation(new URLSearchParams({ page, status: "unknown" })).page).toBe(1);
    }
    expect(reviewLocation(new URLSearchParams()).status).toBe("proposed");
  });
});
