import { CircleNotch, MagnifyingGlass } from "@phosphor-icons/react";
import { Button, Form, Input, InputNumber, Select, Table, type TableColumnsType } from "antd";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { useRequestResult } from "../hooks/useRequestResult.ts";
import type { CollectionListResponse } from "../lib/collections.ts";
import {
  buildSearchRequest,
  extractHits,
  friendlySearchError,
  SEARCH_MODE_META,
  type SearchFormValues,
  type SearchHit,
  type SearchMode,
  searchDocumentPath,
} from "../lib/search.ts";
import type { JsonObject } from "../types.ts";
import { JsonResult } from "./JsonResult.tsx";
import { RequestFeedback } from "./RequestFeedback.tsx";

interface SearchPanelProps {
  api: CogniGraphApi;
  mode: SearchMode;
}

/// One search mode's form + results. Collections come from the tenant's
/// catalog; hits link straight into the document browser.
export function SearchPanel({ api, mode }: SearchPanelProps) {
  const navigate = useNavigate();
  const meta = SEARCH_MODE_META[mode];
  const [collections, setCollections] = useState<string[]>([]);
  const [edgeCollections, setEdgeCollections] = useState<string[]>([]);
  const [values, setValues] = useState<SearchFormValues>({
    query: "",
    collection: "",
    limit: 10,
    threshold: 0.3,
    vectorText: "",
    edgeCollection: undefined,
    maxDepth: 2,
  });
  const { state: result, run: runResult } = useRequestResult<JsonObject>(
    api,
    JSON.stringify([mode, values]),
  );
  const running = result.status === "pending";
  const raw = result.status === "success" ? result.data : undefined;
  const hits = raw ? extractHits(raw) : undefined;
  const elapsed = "elapsed" in result ? result.elapsed : undefined;

  useEffect(() => {
    api
      .get<CollectionListResponse>("/collections")
      .then(({ collections: catalog }) => {
        const docs = catalog.filter((c) => c.collection_type === "document").map((c) => c.name);
        const edges = catalog.filter((c) => c.collection_type === "edge").map((c) => c.name);
        setCollections(docs);
        setEdgeCollections(edges);
        setValues((current) => ({
          ...current,
          collection: current.collection || (docs[0] ?? ""),
          edgeCollection: current.edgeCollection ?? edges[0],
        }));
      })
      .catch(() => {
        setCollections([]);
        setEdgeCollections([]);
      });
  }, [api]);

  const needsQuery = mode !== "vector";
  const ready =
    Boolean(values.collection) &&
    (needsQuery ? Boolean(values.query.trim()) : Boolean(values.vectorText.trim())) &&
    (mode !== "graph-augmented" || Boolean(values.edgeCollection));

  const run = () =>
    runResult(async () => api.post<JsonObject>(meta.endpoint, buildSearchRequest(mode, values)));

  const columns: TableColumnsType<SearchHit> = [
    {
      title: "Score",
      dataIndex: "score",
      width: 110,
      render: (value: number) => value.toFixed(4),
    },
    {
      title: "Document",
      key: "doc",
      render: (_, hit) => {
        const path = searchDocumentPath(hit.document_id);
        return path ? (
          <Link
            aria-label={`Open document ${hit.document_id}`}
            className="table-action mono-cell"
            onClick={(event) => event.stopPropagation()}
            to={path}
          >
            {hit.document_id}
          </Link>
        ) : (
          <span className="mono-cell">{hit.document_id || "—"}</span>
        );
      },
    },
    {
      title: "Title",
      key: "title",
      render: (_, hit) => String(hit.document?.title ?? "—"),
    },
  ];

  return (
    <div className="search-panel">
      <p className="dialog-hint">{meta.hint}</p>
      <Form className="search-form" layout="vertical">
        {needsQuery ? (
          <Form.Item className="search-query-field" label="Query" required>
            <Input.TextArea
              autoSize={{ minRows: 1, maxRows: 3 }}
              onChange={(event) => setValues((v) => ({ ...v, query: event.target.value }))}
              placeholder="What are you looking for?"
              value={values.query}
            />
          </Form.Item>
        ) : (
          <Form.Item className="search-query-field" label="Vector (JSON array)" required>
            <Input.TextArea
              autoSize={{ minRows: 1, maxRows: 4 }}
              onChange={(event) => setValues((v) => ({ ...v, vectorText: event.target.value }))}
              placeholder="[0.021, -0.084, …]"
              value={values.vectorText}
            />
          </Form.Item>
        )}
        <Form.Item label="Collection" required>
          <Select
            disabled={collections.length === 0}
            onChange={(collection) => setValues((v) => ({ ...v, collection }))}
            options={collections.map((name) => ({ label: name, value: name }))}
            placeholder={collections.length === 0 ? "None" : "Select"}
            value={values.collection || undefined}
          />
        </Form.Item>
        {mode === "graph-augmented" ? (
          <>
            <Form.Item label="Edge collection" required>
              <Select
                disabled={edgeCollections.length === 0}
                onChange={(edgeCollection) => setValues((v) => ({ ...v, edgeCollection }))}
                options={edgeCollections.map((name) => ({ label: name, value: name }))}
                placeholder={edgeCollections.length === 0 ? "None in this tenant" : "Select"}
                value={values.edgeCollection}
              />
            </Form.Item>
            <Form.Item label="Max depth">
              <InputNumber
                controls={false}
                max={5}
                min={1}
                onChange={(maxDepth) => setValues((v) => ({ ...v, maxDepth: maxDepth ?? 2 }))}
                value={values.maxDepth}
              />
            </Form.Item>
          </>
        ) : null}
        <Form.Item label="Limit">
          <InputNumber
            controls={false}
            max={100}
            min={1}
            onChange={(limit) => setValues((v) => ({ ...v, limit: limit ?? 10 }))}
            value={values.limit}
          />
        </Form.Item>
        <Form.Item label="Threshold">
          <InputNumber
            controls={false}
            max={1}
            min={0}
            onChange={(threshold) => setValues((v) => ({ ...v, threshold: threshold ?? 0.3 }))}
            step={0.05}
            value={values.threshold}
          />
        </Form.Item>
        <Button
          disabled={running || !ready}
          icon={
            running ? (
              <CircleNotch className="cg-spin" weight="bold" />
            ) : (
              <MagnifyingGlass size={16} />
            )
          }
          onClick={() => void run()}
          type="primary"
        >
          {running ? "Searching…" : "Search"}
        </Button>
      </Form>

      <RequestFeedback
        state={
          result.status === "error"
            ? { ...result, error: friendlySearchError(result.error) }
            : result
        }
      />

      {hits !== undefined ? (
        <section className="search-results">
          <div className="workspace-controls">
            <span className="workspace-count">
              {hits.length} {hits.length === 1 ? "hit" : "hits"}
              {elapsed !== undefined ? ` · ${elapsed} ms` : ""}
            </span>
          </div>
          <Table<SearchHit>
            columns={columns}
            dataSource={hits}
            locale={{ emptyText: "No documents matched — loosen the threshold or rephrase." }}
            onRow={(hit) => ({
              onClick: () => {
                const path = searchDocumentPath(hit.document_id);
                if (path) navigate(path);
              },
            })}
            pagination={false}
            rowClassName={(hit) => (searchDocumentPath(hit.document_id) ? "clickable-row" : "")}
            rowKey="document_id"
            size="small"
          />
          <details className="search-raw">
            <summary>Raw response</summary>
            <JsonResult value={raw} />
          </details>
        </section>
      ) : null}
    </div>
  );
}
