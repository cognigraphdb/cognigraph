import {
  ArrowsClockwise,
  Buildings,
  CircleNotch,
  Pause,
  Play,
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
  Spin,
  Table,
  type TableColumnsType,
  Tag,
  Tooltip,
} from "antd";
import { useCallback, useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { ErrorAlert } from "../components/ErrorAlert.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { TENANT_STATUS_META, type TenantListResponse, type TenantRecord } from "../lib/tenants.ts";
import { formatUnixSeconds } from "../lib/tokens.ts";
import type { Notify } from "../types.ts";

/// /tenants — host-admin only (the sidebar hides it for other roles and the
/// server's TenantAdmin guard enforces it). Manages tenant RECORDS: create,
/// suspend/resume (suspension locks that tenant's users out at the auth
/// gate), delete. Host-admin deliberately has no access to tenant data (D4).
export function TenantsScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const [tenants, setTenants] = useState<TenantRecord[]>([]);
  const [openStores, setOpenStores] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [busyName, setBusyName] = useState<string>();
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await api.get<TenantListResponse>("/tenants");
      setTenants(response.tenants ?? []);
      setOpenStores(response.open_stores ?? 0);
    } catch (reason) {
      setTenants([]);
      setError(reason instanceof Error ? reason.message : "Unable to list tenants");
    } finally {
      setLoading(false);
    }
  }, [api]);

  useEffect(() => void load(), [load]);

  async function setStatus(tenant: TenantRecord, status: "active" | "suspended") {
    setBusyName(tenant.name);
    try {
      await api.post(`/tenants/${encodeURIComponent(tenant.name)}`, { status });
      notify(
        status === "suspended"
          ? `Suspended ${tenant.name} — its users are locked out`
          : `Resumed ${tenant.name}`,
      );
      void load();
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Status change failed", "error");
    } finally {
      setBusyName(undefined);
    }
  }

  async function remove(tenant: TenantRecord) {
    setBusyName(tenant.name);
    try {
      await api.delete(`/tenants/${encodeURIComponent(tenant.name)}`);
      notify(`Deleted tenant record ${tenant.name}`);
      void load();
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Delete failed", "error");
    } finally {
      setBusyName(undefined);
    }
  }

  const columns: TableColumnsType<TenantRecord> = [
    {
      title: "Tenant",
      dataIndex: "name",
      render: (value) => <strong>{String(value)}</strong>,
    },
    {
      title: "Status",
      dataIndex: "status",
      width: 130,
      render: (value: TenantRecord["status"]) => {
        const meta = TENANT_STATUS_META[value];
        return <Tag color={meta?.color}>{meta?.label ?? String(value)}</Tag>;
      },
    },
    {
      title: "Store",
      dataIndex: "store_open",
      width: 110,
      // Stores open lazily: creating a tenant writes only its record; the
      // data store (and its file) appears on the first data access and
      // stays open until the server restarts.
      render: (value: boolean) =>
        value ? (
          <Tooltip title="This tenant's database is loaded in server memory (opened on first data access; stays open until restart).">
            <Tag color="geekblue">Open</Tag>
          </Tooltip>
        ) : (
          <Tooltip title="No data access yet this server run — the store (and its file, on first ever use) opens automatically when a tenant user touches data.">
            <Tag>Closed</Tag>
          </Tooltip>
        ),
    },
    {
      title: "Created",
      dataIndex: "created_at",
      width: 190,
      render: (value: number) => formatUnixSeconds(value),
    },
    {
      title: "",
      key: "actions",
      width: 220,
      render: (_, tenant) => {
        const busy = busyName !== undefined;
        const spinning = busyName === tenant.name;
        return (
          <span className="actions-row-base">
            {tenant.status === "active" ? (
              <Popconfirm
                cancelText="Cancel"
                icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
                okText="Suspend"
                okButtonProps={{ danger: true }}
                onConfirm={() => void setStatus(tenant, "suspended")}
                placement="topLeft"
                title={`Suspend ${tenant.name}? Its users are refused at sign-in immediately.`}
              >
                <Button
                  disabled={busy}
                  icon={
                    spinning ? (
                      <CircleNotch className="cg-spin" weight="bold" />
                    ) : (
                      <Pause size={14} />
                    )
                  }
                  size="small"
                  type="text"
                >
                  Suspend
                </Button>
              </Popconfirm>
            ) : (
              <Button
                disabled={busy}
                icon={
                  spinning ? <CircleNotch className="cg-spin" weight="bold" /> : <Play size={14} />
                }
                onClick={() => void setStatus(tenant, "active")}
                size="small"
                type="text"
              >
                Resume
              </Button>
            )}
            <Popconfirm
              cancelText="Cancel"
              icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
              okText="Delete"
              okButtonProps={{ danger: true }}
              onConfirm={() => void remove(tenant)}
              placement="topLeft"
              title={`Delete the ${tenant.name} record? Its users are refused until it is recreated.`}
            >
              <Button danger disabled={busy} icon={<Trash size={14} />} size="small" type="text">
                Delete
              </Button>
            </Popconfirm>
          </span>
        );
      },
    },
  ];

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <>
            <Button
              disabled={loading || !!error}
              icon={<Plus size={17} />}
              onClick={() => setCreateOpen(true)}
              type="primary"
            >
              Create tenant
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
        description="Tenant lifecycle only: create, suspend, resume, delete records. Host-admin has no access to any tenant's data."
        eyebrow="Access / tenants"
        title="Tenants"
      />
      <section className="user-list">
        <div className="workspace-controls">
          <span className="workspace-count">
            {tenants.length} {tenants.length === 1 ? "tenant" : "tenants"} · {openStores} open{" "}
            {openStores === 1 ? "store" : "stores"}
          </span>
        </div>
        <Spin indicator={<CircleNotch className="cg-spin" weight="bold" />} spinning={loading}>
          {!loading && error ? (
            <Result
              icon={<Buildings aria-hidden="true" size={34} />}
              status="info"
              subTitle={error}
              title="Tenant management is unavailable"
            />
          ) : null}
          {!error ? (
            <Table<TenantRecord>
              columns={columns}
              dataSource={tenants}
              locale={{
                emptyText:
                  "No tenant records — the implicit default tenant needs none until you suspend it.",
              }}
              pagination={false}
              rowKey="name"
              size="small"
            />
          ) : null}
        </Spin>
      </section>

      {createOpen ? (
        <CreateTenantDialog
          api={api}
          notify={notify}
          onClose={() => setCreateOpen(false)}
          onCreated={() => {
            setCreateOpen(false);
            void load();
          }}
        />
      ) : null}
    </main>
  );
}

// Rendered conditionally by the parent (mount = open, unmount = closed).
function CreateTenantDialog({
  api,
  notify,
  onClose,
  onCreated,
}: {
  api: CogniGraphApi;
  notify: Notify;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form] = Form.useForm<{ name: string }>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(values: { name: string }) {
    setBusy(true);
    setError("");
    try {
      await api.post("/tenants", { name: values.name.trim() });
      notify(`Created tenant ${values.name.trim()}`);
      onCreated();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Tenant creation failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Create tenant"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title="Create a tenant"
    >
      <p className="dialog-hint">
        Creates the tenant record; users are then filed under it via user management. Quota fields
        are reserved schema and not yet enforced.
      </p>
      <Form
        form={form}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Tenant name"
          name="name"
          rules={[{ required: true, message: "Name the tenant." }]}
        >
          <Input autoFocus placeholder="acme" />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
