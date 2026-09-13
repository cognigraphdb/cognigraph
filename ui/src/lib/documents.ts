import type { GraphDocument } from "../types.ts";

export interface DocumentFilters {
  search: string;
  category: string;
  embedding: string;
}

export function filterDocuments(
  documents: GraphDocument[],
  { search, category, embedding }: DocumentFilters,
): GraphDocument[] {
  const needle = search.trim().toLocaleLowerCase();

  return documents.filter((document) => {
    const matchesSearch =
      needle.length === 0 ||
      document.title.toLocaleLowerCase().includes(needle) ||
      document._key.toLocaleLowerCase().includes(needle);
    const matchesCategory = category === "all" || document.category === category;
    const matchesEmbedding = embedding === "all" || document.embedding === embedding;

    return matchesSearch && matchesCategory && matchesEmbedding;
  });
}

export function documentJson(document: GraphDocument): string {
  return JSON.stringify(document.raw, null, 2);
}
