import type { CogniGraphApi } from "../api/client.ts";
import type { JsonObject } from "../types.ts";
import {
  NEURON_STATUSES,
  type NeuronDoc,
  type NeuronListResponse,
  type NeuronStatus,
} from "./neurons.ts";

export type StatusFilter = NeuronStatus | "all";
export const REVIEW_PAGE_SIZE = 50;
const SPACE_PAGE_SIZE = 100;

export function reviewLocation(params: URLSearchParams) {
  const status = params.get("status");
  const page = Number(params.get("page") ?? 1);
  return {
    space: params.get("space") || undefined,
    selectedKey: params.get("neuron") || undefined,
    status: (status === "all" || NEURON_STATUSES.includes(status as NeuronStatus)
      ? status
      : "proposed") as StatusFilter,
    page:
      Number.isSafeInteger(page) && page > 0 && page <= Number.MAX_SAFE_INTEGER / REVIEW_PAGE_SIZE
        ? page
        : 1,
  };
}

// Publish the catalog only when every page succeeded. A failed continuation
// must not turn a partial set into an apparently complete list of choices.
export async function loadSpaceTypes(api: CogniGraphApi, signal: AbortSignal): Promise<string[]> {
  const names = new Set<string>();
  for (let offset = 0; ; offset += SPACE_PAGE_SIZE) {
    signal.throwIfAborted();
    const response = await api.request<{ results: JsonObject[] }>(
      `/documents?collection=space_types&limit=${SPACE_PAGE_SIZE}&offset=${offset}`,
      { signal },
    );
    if (!Array.isArray(response.results)) throw new Error("Invalid space catalog response.");
    for (const doc of response.results) {
      if (typeof doc._key !== "string" || !doc._key || names.has(doc._key)) {
        throw new Error("The space catalog changed during loading. Refresh spaces to retry.");
      }
      names.add(doc._key);
    }
    if (response.results.length < SPACE_PAGE_SIZE) return [...names].sort();
  }
}

export interface NeuronPage {
  neurons: NeuronDoc[];
  page: number;
  hasMore: boolean;
}

// The endpoint's count is the returned page length, not a dataset total.
// One lookahead row establishes whether Next is available without inventing a total.
export async function loadNeuronPage(
  api: CogniGraphApi,
  space: string,
  status: StatusFilter,
  requestedPage: number,
  signal: AbortSignal,
): Promise<NeuronPage> {
  const params = new URLSearchParams({ space_type: space, limit: String(REVIEW_PAGE_SIZE + 1) });
  if (status !== "all") params.set("status", status);
  let page = requestedPage;
  for (;;) {
    signal.throwIfAborted();
    params.set("offset", String((page - 1) * REVIEW_PAGE_SIZE));
    const response = await api.request<NeuronListResponse>(`/neurons?${params}`, { signal });
    if (!Array.isArray(response.neurons)) throw new Error("Invalid neuron queue response.");
    if (response.neurons.length || page === 1) {
      return {
        page,
        neurons: response.neurons.slice(0, REVIEW_PAGE_SIZE),
        hasMore: response.neurons.length > REVIEW_PAGE_SIZE,
      };
    }
    // A verdict may remove the final row of the last page. A stale deep link
    // can be arbitrarily far out of range; return to the first page in one retry.
    page = 1;
  }
}

export function neuronPageLabel(page: NeuronPage): string {
  if (!page.neurons.length) return "0 neurons";
  const start = (page.page - 1) * REVIEW_PAGE_SIZE + 1;
  return `${start}–${start + page.neurons.length - 1} shown · ${page.hasMore ? "More available" : "End of queue"}`;
}

export async function loadSelectedNeuron(
  api: CogniGraphApi,
  space: string,
  key: string,
  signal: AbortSignal,
) {
  const neuron = await api.request<NeuronDoc>(`/documents/neurons/${encodeURIComponent(key)}`, {
    signal,
  });
  if (neuron._key !== key || neuron.space_type !== space) {
    throw new Error("The selected neuron does not belong to this space.");
  }
  return neuron;
}
