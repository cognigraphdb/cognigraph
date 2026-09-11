// Collection catalog domain model. Mirrors GET /api/collections
// (crates/cognigraph-server/src/routes/collections.rs): system collections
// (underscore prefix) are already filtered server-side.

export interface CollectionInfo {
  name: string;
  collection_type: "document" | "edge";
  count: number;
}

export interface CollectionListResponse {
  collections: CollectionInfo[];
  count: number;
}

export const COLLECTION_TYPE_META: Record<
  CollectionInfo["collection_type"],
  { label: string; color: string }
> = {
  document: { label: "Documents", color: "geekblue" },
  edge: { label: "Edges", color: "purple" },
};

/// Edge collections hold relationships, not standalone documents — the
/// document browser only opens document collections.
export function isBrowsable(info: CollectionInfo): boolean {
  return info.collection_type === "document";
}
