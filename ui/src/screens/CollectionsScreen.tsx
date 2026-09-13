import { Button, Spin } from "antd";
import { useEffect, useState } from "react";
import { useParams, useSearchParams } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { CollectionToolbar } from "../components/CollectionToolbar.tsx";
import { DocumentDialog } from "../components/DocumentDialog.tsx";
import { DocumentInspector } from "../components/DocumentInspector.tsx";
import { DocumentTable } from "../components/DocumentTable.tsx";
import { ErrorAlert } from "../components/ErrorAlert.tsx";
import { useCollectionSearch } from "../hooks/useCollectionSearch.ts";
import { embedDocumentRequest, normalizeDocument } from "../lib/api-documents.ts";
import { COLLECTION_TEXT_LIMIT, collectionView } from "../lib/collection-search.ts";
import type { DocumentEdit } from "../lib/document-edit.ts";
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
  const { dataWrite } = useAccess();
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
  const serverQuery = search.trim();
  const connected = connection !== "offline";
  const searchState = useCollectionSearch(api, collection, serverQuery, connection);
  const [loadedPage, setLoadedPage] = useState("");
  const [listError, setListError] = useState("");
  const [listRevision, setListRevision] = useState(0);
  const listOwner = JSON.stringify([collection, page, pageSize, listRevision]);

  if (pagedCollection !== collection) {
    setPagedCollection(collection);
    setPage(1);
    setSearch(targetKey ?? "");
    setCategory("all");
    setEmbedding("all");
    setDocuments([]);
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
    if (!connected || serverQuery) {
      setLoading(false);
      return;
    }
    let active = true;
    setLoading(true);
    setListError("");
    setLoadedPage("");
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
        setLoadedPage(listOwner);
        setSelectedKey((current) => {
          if (current && next.some((document) => document._key === current)) {
            return current;
          }
          return next[0]?._key;
        });
      })
      .catch((error: Error) => {
        if (active) setListError(`Documents: ${error.message}`);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [api, collection, connected, page, pageSize, serverQuery, listOwner]);

  useEffect(() => {
    if (!connected) return;
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
  }, [api, collection, connected]);

  const activeDocs = serverQuery
    ? (searchState.result?.documents ?? [])
    : loadedPage === listOwner
      ? documents
      : [];
  const view = collectionView(
    activeDocs,
    { category, embedding },
    {
      page,
      pageSize,
      search: Boolean(serverQuery),
      total: totalCount,
    },
  );
  const busy = serverQuery ? searchState.searching : loading;
  const errors = serverQuery ? (searchState.result?.errors ?? []) : listError ? [listError] : [];
  const selected =
    view.visible.find((document) => document._key === selectedKey) ?? view.visible[0];
  const filterChanged = (set: (value: string) => void) => (value: string) => {
    set(value);
    if (serverQuery) setPage(1);
  };

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
      setTotalCount((current) => (current === undefined ? undefined : current + 1));
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
    searchState.updateDocuments(replace);
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
      searchState.updateDocuments(replace);
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
      searchState.updateDocuments(drop);
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
          categories={view.categories}
          category={category}
          collection={collection}
          documentCount={totalCount}
          embedding={embedding}
          onCategory={filterChanged(setCategory)}
          onCreate={() => setDialog("create")}
          onEmbedding={filterChanged(setEmbedding)}
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
        <div className="collection-scope" aria-live="polite">
          {serverQuery ? (
            <>
              <p>
                Search retrieves up to {COLLECTION_TEXT_LIMIT} text matches plus an exact key.
                Category choices and filters apply only to retrieved documents.
              </p>
              {searchState.result?.textLimitReached ? (
                <p>
                  <strong>Text limit reached; more matches may exist.</strong> Refine the search to
                  narrow the results.
                </p>
              ) : null}
            </>
          ) : (
            <p>
              Category choices and filters apply only to the loaded collection page. Page controls
              browse the full collection.
            </p>
          )}
          {errors.length ? (
            <ErrorAlert title={`Results may be incomplete. ${errors.join(" ")}`} />
          ) : null}
          {errors.length ? (
            <Button
              onClick={
                serverQuery ? searchState.retry : () => setListRevision((current) => current + 1)
              }
              size="small"
            >
              {serverQuery ? "Retry search" : "Retry collection"}
            </Button>
          ) : null}
        </div>
        {busy ? <Spin className="panel-loading" size="small" /> : null}
        <DocumentTable
          documents={view.visible}
          onPage={setPage}
          onPageSize={changePageSize}
          onSelect={selectDocument}
          page={view.page}
          pageSize={pageSize}
          selectedKey={selected?._key}
          summary={
            busy
              ? "Loading documents…"
              : !serverQuery && listError
                ? "Collection page unavailable"
                : view.summary
          }
          hasNextPage={view.hasNext}
          busy={busy}
          emptyDescription={
            busy
              ? "Loading documents…"
              : errors.length
                ? "The request failed. Use Retry above to load this view."
                : "Change or reset the filters, or browse another collection page."
          }
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
      {dialog === "create" && dataWrite ? (
        <DocumentDialog
          collection={collection}
          mode="create"
          onCancel={() => setDialog(null)}
          onCreate={createDocument}
        />
      ) : null}
      {dialog === "delete" && selected && dataWrite ? (
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
