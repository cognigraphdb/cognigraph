import {
  ArrowsClockwise,
  CircleNotch,
  Database,
  Plus,
  Trash,
  WarningCircle,
} from "@phosphor-icons/react";
import {
  Button,
  Form,
  Input,
  Modal,
  Popconfirm,
  Result,
  Select,
  Spin,
  Table,
  type TableColumnsType,
  Tag,
} from "antd";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { ErrorAlert } from "../components/ErrorAlert.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import {
  COLLECTION_TYPE_META,
  type CollectionInfo,
  type CollectionListResponse,
  isBrowsable,
} from "../lib/collections.ts";
import type { Notify } from "../types.ts";

/// /collections — the catalog. Rows open the document browser at
/// /collections/{name}; edge collections are listed but not browsable
/// (relationships live on the Graph page).
export function CollectionsIndexScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const { dataWrite } = useAccess();
  const navigate = useNavigate();
  const [collections, setCollections] = useState<CollectionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [droppingName, setDroppingName] = useState<string>();

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await api.get<CollectionListResponse>("/collections");
      setCollections(response.collections ?? []);
    } catch (reason) {
      setCollections([]);
      setError(reason instanceof Error ? reason.message : "Unable to list collections");
    } finally {
      setLoading(false);
    }
  }, [api]);

  useEffect(() => void load(), [load]);

  async function dropCollection(info: CollectionInfo) {
    if (!dataWrite) return;
    setDroppingName(info.name);
    try {
      await api.delete(`/collections/${encodeURIComponent(info.name)}`);
      notify(`Dropped ${info.name}`);
      void load();
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Drop failed", "error");
    } finally {
      setDroppingName(undefined);
    }
  }

  const columns: TableColumnsType<CollectionInfo> = [
    {
      title: "Collection",
      dataIndex: "name",
      render: (_, info) =>
        isBrowsable(info) ? (
          <Link
            aria-label={`Open collection ${info.name}`}
            className="table-action mono-cell"
            onClick={(event) => event.stopPropagation()}
            to={`/collections/${encodeURIComponent(info.name)}`}
          >
            <strong>{info.name}</strong>
          </Link>
        ) : (
          <strong className="mono-cell">{info.name}</strong>
        ),
    },
    {
      title: "Type",
      dataIndex: "collection_type",
      width: 140,
      render: (value: CollectionInfo["collection_type"]) => {
        const meta = COLLECTION_TYPE_META[value];
        return <Tag color={meta?.color}>{meta?.label ?? String(value)}</Tag>;
      },
    },
    {
      title: "Entries",
      dataIndex: "count",
      width: 130,
      render: (value: number) => value.toLocaleString(),
    },
    {
      title: "",
      key: "actions",
      width: 110,
      render: (_, info) =>
        dataWrite ? (
          <Popconfirm
            cancelText="Cancel"
            icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
            okButtonProps={{ danger: true }}
            okText="Drop collection"
            onConfirm={() => void dropCollection(info)}
            placement="topLeft"
            title={`Drop ${info.name} and its ${info.count.toLocaleString()} ${
              info.count === 1 ? "entry" : "entries"
            }? This cannot be undone.`}
          >
            <Button
              danger
              disabled={droppingName !== undefined}
              icon={
                droppingName === info.name ? (
                  <CircleNotch className="cg-spin" weight="bold" />
                ) : (
                  <Trash size={14} />
                )
              }
              size="small"
              type="text"
            >
              Delete
            </Button>
          </Popconfirm>
        ) : null,
    },
  ];

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <>
            <Button
              disabled={!dataWrite || loading || !!error}
              icon={<Plus size={17} />}
              onClick={() => setCreateOpen(true)}
              type="primary"
            >
              Create collection
            </Button>
            <Button
              disabled={loading}
              icon={
                loading ? (
                  <CircleNotch className="cg-spin" weight="bold" />
                ) : (
                  <ArrowsClockwise size={17} />
                )
              }
              onClick={() => void load()}
            >
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
          </>
        }
        description={
          dataWrite
            ? "Browse and edit application documents; explore edge collections on the Graph page."
            : "Read-only access to application collections. Your role cannot create, edit or delete data."
        }
        eyebrow="Data / collections"
        title="Collections"
      />
      <section className="user-list">
        <div className="workspace-controls">
          <span className="workspace-count">
            {collections.length} {collections.length === 1 ? "collection" : "collections"}
          </span>
        </div>
        <Spin indicator={<CircleNotch className="cg-spin" weight="bold" />} spinning={loading}>
          {!loading && error ? (
            <Result
              icon={<Database aria-hidden="true" size={34} />}
              status="info"
              subTitle={error}
              title="The collection catalog is unavailable"
            />
          ) : null}
          {!error ? (
            <Table<CollectionInfo>
              columns={columns}
              dataSource={collections}
              locale={{ emptyText: "No collections yet — create a document to start one." }}
              onRow={(info) => ({
                onClick: (event) => {
                  // Row clicks navigate; clicks on the row's own controls
                  // (delete button, popconfirm) must not.
                  if ((event.target as HTMLElement).closest("button, .ant-popover")) return;
                  if (isBrowsable(info)) {
                    navigate(`/collections/${encodeURIComponent(info.name)}`);
                  } else {
                    notify("Edge collections are explored on the Graph page", "info");
                  }
                },
              })}
              pagination={false}
              rowClassName={(info) => (isBrowsable(info) ? "clickable-row" : "")}
              rowKey="name"
              size="small"
            />
          ) : null}
        </Spin>
      </section>

      {createOpen && dataWrite ? (
        <CreateCollectionDialog
          api={api}
          notify={notify}
          onClose={() => setCreateOpen(false)}
          onCreated={(name, type) => {
            setCreateOpen(false);
            if (type === "document") {
              // Land straight in the (empty) browser for the new collection.
              navigate(`/collections/${encodeURIComponent(name)}`);
            } else {
              void load();
            }
          }}
        />
      ) : null}
    </main>
  );
}

// Rendered conditionally by the parent (mount = open, unmount = closed).
function CreateCollectionDialog({
  api,
  notify,
  onClose,
  onCreated,
}: {
  api: CogniGraphApi;
  notify: Notify;
  onClose: () => void;
  onCreated: (name: string, type: "document" | "edge") => void;
}) {
  const [form] = Form.useForm<{ name: string; collection_type: "document" | "edge" }>();
  const [busy, setBusy] = useState(false);
  const [dialogError, setDialogError] = useState("");

  async function submit(values: { name: string; collection_type: "document" | "edge" }) {
    setBusy(true);
    setDialogError("");
    try {
      const name = values.name.trim();
      await api.post("/collections", { name, collection_type: values.collection_type });
      notify(`Created ${name}`);
      onCreated(name, values.collection_type);
    } catch (reason) {
      setDialogError(reason instanceof Error ? reason.message : "Collection creation failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Create collection"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title="Create a collection"
    >
      <p className="dialog-hint">
        Names starting with <code>_</code> are reserved for system collections. Edge collections
        hold relationships and are explored on the Graph page.
      </p>
      <Form
        form={form}
        initialValues={{ collection_type: "document" }}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Name"
          name="name"
          rules={[
            { required: true, message: "Name the collection." },
            {
              pattern: /^[^_/][^/]*$/,
              message: "No leading underscore and no `/` characters.",
            },
          ]}
        >
          <Input autoFocus placeholder="trials" />
        </Form.Item>
        <Form.Item label="Type" name="collection_type" rules={[{ required: true }]}>
          <Select
            options={[
              { label: "Documents — JSON records you browse and edit", value: "document" },
              { label: "Edges — relationships between documents", value: "edge" },
            ]}
          />
        </Form.Item>
      </Form>
      {dialogError ? <ErrorAlert title={dialogError} /> : null}
    </Modal>
  );
}
