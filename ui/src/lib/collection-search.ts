import { ApiError, type CogniGraphApi } from "../api/client.ts";
import type { GraphDocument, JsonObject } from "../types.ts";
import { normalizeDocument } from "./api-documents.ts";
import { type DocumentFilters, filterDocuments } from "./documents.ts";

export const COLLECTION_TEXT_LIMIT = 100;

export interface CollectionSearchResult {
  documents: GraphDocument[];
  textLimitReached: boolean;
  errors: string[];
}

// The API has a limit, but no offset/cursor or total-match count. An exact key
// can add one document outside that bounded text result set.
export async function loadCollectionSearch(
  api: CogniGraphApi,
  collection: string,
  query: string,
): Promise<CollectionSearchResult> {
  const [key, text] = await Promise.allSettled([
    api
      .get<JsonObject>(`/documents/${encodeURIComponent(collection)}/${encodeURIComponent(query)}`)
      .catch((error: unknown) => {
        if (error instanceof ApiError && error.status === 404) return null;
        throw error;
      }),
    api.post<{ results: Array<{ document: JsonObject }> }>("/search/text", {
      collection,
      query,
      fields: ["title", "content", "summary", "text"],
      limit: COLLECTION_TEXT_LIMIT,
    }),
  ]);
  const hits = text.status === "fulfilled" ? text.value.results : [];
  const documents = hits.map((hit) => normalizeDocument(hit.document, collection));
  if (key.status === "fulfilled" && key.value) {
    const exact = normalizeDocument(key.value, collection);
    documents.unshift(exact);
  }
  const errors = [key, text].flatMap((result, index) =>
    result.status === "rejected"
      ? [
          `${index === 0 ? "Exact-key lookup" : "Text search"} failed: ${result.reason instanceof Error ? result.reason.message : "request failed"}`,
        ]
      : [],
  );
  return {
    documents: documents.filter(
      (doc, index) => documents.findIndex((other) => other._key === doc._key) === index,
    ),
    textLimitReached: hits.length >= COLLECTION_TEXT_LIMIT,
    errors,
  };
}

export function collectionView(
  documents: GraphDocument[],
  filters: Omit<DocumentFilters, "search">,
  paging: { page: number; pageSize: number; search: boolean; total?: number },
) {
  const filtered = filterDocuments(documents, { ...filters, search: "" });
  const isFiltered = filters.category !== "all" || filters.embedding !== "all";
  const { pageSize, search, total } = paging;
  const page = search
    ? Math.min(paging.page, Math.max(1, Math.ceil(filtered.length / pageSize)))
    : paging.page;
  const start = (page - 1) * pageSize;
  const visible = search ? filtered.slice(start, start + pageSize) : filtered;
  const categories = new Set(documents.map((doc) => doc.category));
  // A page-local choice remains identifiable when another page lacks it.
  if (filters.category !== "all") categories.add(filters.category);
  const range = visible.length ? `${start + 1}–${start + visible.length}` : "0";
  const summary = search
    ? `${range} of ${filtered.length} ${isFiltered ? "filtered " : ""}retrieved`
    : isFiltered
      ? `${visible.length} of ${documents.length} on this page`
      : `${range}${total === undefined ? " loaded" : ` of ${total.toLocaleString()} documents`}`;
  return {
    visible,
    page,
    categories: [...categories].sort(),
    summary,
    hasNext: search
      ? page * pageSize < filtered.length
      : total === undefined
        ? documents.length >= pageSize
        : page * pageSize < total,
  };
}
