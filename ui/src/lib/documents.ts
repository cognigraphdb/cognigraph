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
  return JSON.stringify(
    {
      _id: document._id,
      _key: document._key,
      title: document.title,
      category: document.category,
      tags: document.tags,
      summary: document.summary,
      owner: document.owner,
      createdAt: document.createdAt,
      updatedAt: document.updatedAt,
      content: document.content,
    },
    null,
    2,
  );
}
