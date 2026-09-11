import { Spin } from "antd";
import { useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { CollectionToolbar } from "../components/CollectionToolbar.tsx";
import { DocumentDialog } from "../components/DocumentDialog.tsx";
import { DocumentInspector } from "../components/DocumentInspector.tsx";
import { DocumentTable } from "../components/DocumentTable.tsx";
import { embedDocumentRequest, normalizeDocument } from "../lib/api-documents.ts";
import type { DocumentEdit } from "../lib/document-edit.ts";
import { filterDocuments } from "../lib/documents.ts";
import type {
  ConnectionStatus,
  DocumentDraft,
  GraphDocument,
  JsonObject,
  Notify,
} from "../types.ts";

interface CollectionsScreenProps {
  api: CogniGraphApi;
  connection: ConnectionStatus;
  notify: Notify;
}

export function CollectionsScreen({ api, connection, notify }: CollectionsScreenProps) {
  // The browsed collection comes from the URL (/collections/{collection});
  // An optional ?doc={key} enters whole-collection search, so off-page
  // documents are resolved by key instead of silently selecting the first row.
  const { collection = "documents" } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const targetKey = searchParams.get("doc") ?? undefined;
  const [documents, setDocuments] = useState<GraphDocument[]>([]);
  // The collection's true size comes from the catalog — the page shows at
  // most `pageSize` documents, and the heading must not conflate the two.
  const [totalCount, setTotalCount] = useState<number>();
  const [selectedKey, setSelectedKey] = useState<string>();
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [search, setSearch] = useState(targetKey ?? "");
  const [category, setCategory] = useState("all");
  const [embedding, setEmbedding] = useState("all");
  const [pageSize, setPageSize] = useState(25);
  const [page, setPage] = useState(1);
  // Reset paging when the browsed collection changes (React's render-time
  // derived-state reset — the component instance survives route changes).
  const [pagedCollection, setPagedCollection] = useState(collection);
  const changePageSize = (size: number) => {
    setPageSize(size);
    setPage(1);
  };
  const [dialog, setDialog] = useState<"create" | "delete" | null>(null);
  const [loading, setLoading] = useState(false);
  // Server-side search: a non-empty query switches the table from the paged
  // listing to /api/search/text hits (plus an exact-key lookup), so matches
  // come from the whole collection, not just the loaded page.
  const [searchResults, setSearchResults] = useState<GraphDocument[]>();
  const [searching, setSearching] = useState(false);
  const serverQuery = search.trim();

  if (pagedCollection !== collection) {
    setPagedCollection(collection);
    setPage(1);
    setSearch(targetKey ?? "");
    setCategory("all");
    setEmbedding("all");
    setDocuments([]);
    setSearchResults(undefined);
    setTotalCount(undefined);
    setSelectedKey(undefined);
    setInspectorOpen(true);
    setDialog(null);
  }

  useEffect(() => {
    if (!targetKey) return;
    setSearch(targetKey);
    setPage(1);
    setInspectorOpen(true);
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("doc");
        return next;
      },
      { replace: true },
    );
  }, [targetKey, setSearchParams]);

  useEffect(() => {
    if (connection !== "online") return;
    let active = true;
    setSearching(false);
    if (!serverQuery) {
      setSearchResults(undefined);
      return;
    }
    const timer = setTimeout(() => {
      setSearching(true);
      const byKey = api
        .get<JsonObject>(
          `/documents/${encodeURIComponent(collection)}/${encodeURIComponent(serverQuery)}`,
        )
        .then((doc) => normalizeDocument(doc, collection))
        .catch(() => null);
      const byText = api
        .post<{ results: Array<{ document: JsonObject }> }>("/search/text", {
          collection,
          query: serverQuery,
          fields: ["title", "content", "summary", "text"],
          limit: 100,
        })
        .then(({ results }) => results.map((hit) => normalizeDocument(hit.document, collection)))
        .catch((error: Error) => {
          if (active) notify(`Search: ${error.message}`, "error");
          return [] as GraphDocument[];
        });
      Promise.all([byKey, byText])
        .then(([keyDoc, hits]) => {
          if (!active) return;
          const merged = keyDoc
            ? [keyDoc, ...hits.filter((hit) => hit._key !== keyDoc._key)]
            : hits;
          setSearchResults(merged);
          setSelectedKey((current) =>
            current && merged.some((doc) => doc._key === current) ? current : merged[0]?._key,
          );
        })
        .finally(() => {
          if (active) setSearching(false);
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [api, collection, connection, notify, serverQuery]);

  useEffect(() => {
    if (connection !== "online" || serverQuery) {
      setLoading(false);
      return;
    }
    let active = true;
    setLoading(true);
    api
      .get<{ results: JsonObject[] }>(
        `/documents?collection=${encodeURIComponent(collection)}&limit=${pageSize}&offset=${
          (page - 1) * pageSize
        }`,
      )
      .then(({ results }) => {
        if (!active) return;
        const next = results.map((item) => normalizeDocument(item, collection));
        setDocuments(next);
        setSelectedKey((current) => {
          if (current && next.some((document) => document._key === current)) {
            return current;
          }
          return next[0]?._key;
        });
      })
      .catch((error: Error) => {
        if (active) notify(`Documents: ${error.message}`, "error");
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [api, collection, connection, notify, page, pageSize, serverQuery]);

  useEffect(() => {
    if (connection !== "online") return;
    let active = true;
    api
      .get<{ collections: Array<{ name: string; count: number }> }>("/collections")
      .then(({ collections }) => {
        if (!active) return;
        setTotalCount(collections.find((info) => info.name === collection)?.count);
      })
      .catch(() => {
        if (active) setTotalCount(undefined);
      });
    return () => {
      active = false;
    };
  }, [api, collection, connection]);

  // Search mode shows server hits (text matching already done); the category
  // and embedding filters still apply client-side on top of either source.
  const activeDocs = searchResults ?? documents;
  const filtered = useMemo(
    () =>
      filterDocuments(activeDocs, {
        search: searchResults ? "" : search,
        category,
        embedding,
      }),
    [activeDocs, searchResults, search, category, embedding],
  );
  // Search hits span the whole collection, so the table pages them locally;
  // the plain listing is already one server page.
  const visible = searchResults ? filtered.slice((page - 1) * pageSize, page * pageSize) : filtered;
  const selected = activeDocs.find((document) => document._key === selectedKey);
  const categories = [...new Set(activeDocs.map((document) => document.category))].sort();

  const createDocument = async (draft: DocumentDraft) => {
    try {
      const now = new Date().toISOString();
      const payload = {
        collection,
        ...draft,
        owner: "admin@cognigraph.local",
        tags: [],
        createdAt: now,
        updatedAt: now,
        embedding_status: "missing",
        content: { status: "draft" },
      };
      const created = await api.post<JsonObject>("/documents", payload);
      // POST returns an identity receipt, not the stored JSON. Read it back
      // before exposing the inspector; request metadata is not document data.
      setTotalCount((current) => (current ?? 0) + 1);
      setDialog(null);
      const key = String(created._key);
      let stored: JsonObject;
      try {
        stored = await api.get<JsonObject>(
          `/documents/${encodeURIComponent(collection)}/${encodeURIComponent(key)}`,
        );
      } catch (error) {
        notify(
          `Document ${key} was created, but could not be loaded. Refresh the collection: ${error instanceof Error ? error.message : "read failed"}`,
          "warning",
        );
        return;
      }
      const document = normalizeDocument(stored, collection);
      setDocuments((current) => [document, ...current]);
      setSelectedKey(document._key);
      setInspectorOpen(true);
      notify("Document created through the API");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Document creation failed", "error");
    }
  };

  const updateDocument = async (updated: DocumentEdit) => {
    if (Object.keys(updated.patch).length === 0) return;
    const saved = await api.patch<JsonObject>(
      `/documents/${encodeURIComponent(collection)}/${encodeURIComponent(updated.key)}`,
      updated.patch,
    );
    const normalized = normalizeDocument(saved, collection);
    const replace = (current: GraphDocument[]) =>
      current.map((document) => (document._key === normalized._key ? normalized : document));
    setDocuments(replace);
    setSearchResults((current) => (current ? replace(current) : current));
    notify("Document saved through the API");
  };

  const embedDocument = async () => {
    if (!selected) return;
    try {
      const response = await api.post<{ model?: string }>(
        "/documents/embed",
        embedDocumentRequest(collection, selected),
      );
      // The server stored the vector; show the document as it now is.
      const saved = await api.get<JsonObject>(
        `/documents/${encodeURIComponent(collection)}/${encodeURIComponent(selected._key)}`,
      );
      const normalized = normalizeDocument(saved, collection);
      const replace = (current: GraphDocument[]) =>
        current.map((document) => (document._key === normalized._key ? normalized : document));
      setDocuments(replace);
      setSearchResults((current) => (current ? replace(current) : current));
      notify(`Embedded through ${response.model ?? "the configured provider"}`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "Embedding failed", "error");
    }
  };

  const deleteDocument = async () => {
    if (!selected) return;
    try {
      await api.delete(
        `/documents/${encodeURIComponent(collection)}/${encodeURIComponent(selected._key)}`,
      );
      const drop = (current: GraphDocument[]) =>
        current.filter((document) => document._key !== selected._key);
      setDocuments(drop);
      setSearchResults((current) => (current ? drop(current) : current));
      setTotalCount((current) => (current === undefined ? undefined : Math.max(0, current - 1)));
      setSelectedKey(undefined);
      setInspectorOpen(false);
      setDialog(null);
      notify("Document deleted through the API");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Document deletion failed", "error");
    }
  };

  return (
    <main className={inspectorOpen && selected ? "workspace with-inspector" : "workspace"}>
      <section className="collection-panel">
        <CollectionToolbar
          categories={categories}
          category={category}
          collection={collection}
          documentCount={totalCount ?? documents.length}
          embedding={embedding}
          onCategory={setCategory}
          onCreate={() => setDialog("create")}
          onEmbedding={setEmbedding}
          onReset={() => {
            setPage(1);
            setSearch("");
            setCategory("all");
            setEmbedding("all");
          }}
          onSearch={(value) => {
            setPage(1);
            setSearch(value);
          }}
          search={search}
        />
        {loading || searching ? <Spin className="panel-loading" size="small" /> : null}
        <DocumentTable
          documents={visible}
          onPage={setPage}
          onPageSize={changePageSize}
          onSelect={selectDocument}
          page={page}
          pageSize={pageSize}
          selectedKey={selectedKey}
          total={searchResults ? filtered.length : totalCount}
        />
      </section>
      {inspectorOpen && selected ? (
        <DocumentInspector
          key={selected._id}
          document={selected}
          onClose={() => setInspectorOpen(false)}
          onDelete={() => setDialog("delete")}
          onEmbed={embedDocument}
          onUpdate={updateDocument}
        />
      ) : null}
      {dialog === "create" ? (
        <DocumentDialog
          collection={collection}
          mode="create"
          onCancel={() => setDialog(null)}
          onCreate={createDocument}
        />
      ) : null}
      {dialog === "delete" && selected ? (
        <DocumentDialog
          collection={collection}
          mode="delete"
          onCancel={() => setDialog(null)}
          onDelete={deleteDocument}
          title={selected.title}
        />
      ) : null}
    </main>
  );

  function selectDocument(document: GraphDocument) {
    setSelectedKey(document._key);
    setInspectorOpen(true);
  }
}
