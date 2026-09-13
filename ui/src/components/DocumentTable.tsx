import {
  CaretLeft,
  CaretRight,
  CheckCircle,
  Circle,
  CircleNotch,
  MagnifyingGlass,
  RadioButton,
} from "@phosphor-icons/react";
import { Button, Empty, Select, Table, type TableColumnsType } from "antd";
import type { GraphDocument } from "../types.ts";

interface DocumentTableProps {
  documents: GraphDocument[];
  pageSize: number;
  selectedKey?: string;
  onPageSize: (pageSize: number) => void;
  page: number;
  onPage: (page: number) => void;
  summary: string;
  hasNextPage: boolean;
  busy: boolean;
  emptyDescription: string;
  onSelect: (document: GraphDocument) => void;
}

const dateFormatter = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : dateFormatter.format(date);
}

function EmbeddingIcon({ state }: { state: GraphDocument["embedding"] }) {
  if (state === "ready") {
    return <CheckCircle aria-label="Embedding ready" className="ready-icon" size={18} />;
  }
  if (state === "processing") {
    return <CircleNotch aria-label="Embedding processing" className="processing-icon" size={18} />;
  }
  return <Circle aria-label="Embedding missing" className="missing-icon" size={18} />;
}

export function DocumentTable({
  documents,
  pageSize,
  selectedKey,
  onPageSize,
  page,
  onPage,
  summary,
  hasNextPage,
  busy,
  emptyDescription,
  onSelect,
}: DocumentTableProps) {
  // Column order matches the CSS's positional th:nth-child width rules
  // (components.css) -- selector, key, title, category, updated, embedding.
  const columns: TableColumnsType<GraphDocument> = [
    {
      key: "selector",
      className: "selector-column",
      title: <span className="sr-only">Select</span>,
      width: 42,
      render: (_, document) => {
        const selected = document._key === selectedKey;
        return (
          <Button
            aria-label={`Select ${document.title}`}
            className={selected ? "row-selector selected" : "row-selector"}
            icon={
              <RadioButton aria-hidden="true" size={18} weight={selected ? "fill" : "regular"} />
            }
            onClick={() => onSelect(document)}
            type="text"
          />
        );
      },
    },
    {
      key: "key",
      title: "Key",
      width: 145,
      render: (_, document) => (
        <Button className="key-button" onClick={() => onSelect(document)} type="link">
          {document._key}
        </Button>
      ),
    },
    {
      key: "title",
      title: "Title",
      className: "document-title",
      dataIndex: "title",
      width: "31%",
    },
    {
      key: "category",
      title: "Category",
      dataIndex: "category",
      width: "18%",
    },
    {
      key: "updated",
      title: "Updated",
      width: 180,
      render: (_, document) => formatDate(document.updatedAt),
    },
    {
      key: "embedding",
      title: "Embedding",
      className: "embedding-column embedding-cell",
      width: 82,
      render: (_, document) => <EmbeddingIcon state={document.embedding} />,
    },
  ];

  return (
    <div className="table-region">
      <div className="table-scroll">
        <Table<GraphDocument>
          columns={columns}
          dataSource={documents}
          locale={{
            emptyText: (
              <Empty
                className="empty-state-base empty-state"
                description={emptyDescription}
                image={<MagnifyingGlass aria-hidden="true" size={30} />}
              >
                <strong>No documents in this view</strong>
              </Empty>
            ),
          }}
          pagination={false}
          rowClassName={(document) => (document._key === selectedKey ? "selected" : "")}
          rowKey="_key"
          size="small"
          tableLayout="fixed"
        />
      </div>
      <footer className="pagination">
        <span>Rows per page:</span>
        <Select
          aria-label="Rows per page"
          onChange={onPageSize}
          options={[25, 50, 100].map((value) => ({ label: value, value }))}
          size="small"
          value={pageSize}
        />
        <span className="pagination-summary">{summary}</span>
        <Button
          aria-label="Previous page"
          disabled={busy || page <= 1}
          icon={<CaretLeft aria-hidden="true" size={16} />}
          onClick={() => onPage(page - 1)}
          size="small"
        />
        <Button className="current-page" size="small">
          {page}
        </Button>
        <Button
          aria-label="Next page"
          disabled={busy || !hasNextPage}
          icon={<CaretRight aria-hidden="true" size={16} />}
          onClick={() => onPage(page + 1)}
          size="small"
        />
      </footer>
    </div>
  );
}
