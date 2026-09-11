import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import { Button, Input, Select } from "antd";
import { Link } from "react-router";

interface CollectionToolbarProps {
  collection: string;
  search: string;
  category: string;
  embedding: string;
  categories: string[];
  documentCount: number;
  onSearch: (value: string) => void;
  onCategory: (value: string) => void;
  onEmbedding: (value: string) => void;
  onCreate: () => void;
  onReset: () => void;
}

export function CollectionToolbar(props: CollectionToolbarProps) {
  return (
    <>
      <div className="workspace-header-base collection-heading">
        <div>
          <div className="breadcrumb">
            <Link to="/collections">Collections</Link>&nbsp;&nbsp;/&nbsp;&nbsp;{props.collection}
          </div>
          <div className="title-row">
            <h1>{props.collection}</h1>
          </div>
          <p>
            JSON collection&nbsp;&nbsp;•&nbsp;&nbsp;{props.documentCount.toLocaleString()} documents
          </p>
        </div>
        <Button
          icon={<Plus aria-hidden="true" size={18} />}
          onClick={props.onCreate}
          type="primary"
        >
          Create document
        </Button>
      </div>

      <div className="filters">
        <Input
          allowClear
          aria-label="Search documents"
          onChange={(event) => props.onSearch(event.target.value)}
          placeholder="Search the whole collection, or paste a key..."
          prefix={<MagnifyingGlass aria-hidden="true" />}
          value={props.search}
        />
        <Select
          aria-label="Filter by category"
          onChange={props.onCategory}
          options={[
            { label: "Category: All", value: "all" },
            ...props.categories.map((category) => ({ label: category, value: category })),
          ]}
          value={props.category}
        />
        <Select
          aria-label="Filter by embedding status"
          onChange={props.onEmbedding}
          options={[
            { label: "Has embedding: All", value: "all" },
            { label: "Ready", value: "ready" },
            { label: "Processing", value: "processing" },
            { label: "Missing", value: "missing" },
          ]}
          value={props.embedding}
        />
        <Button className="reset-button" onClick={props.onReset} type="link">
          Reset
        </Button>
      </div>
    </>
  );
}
