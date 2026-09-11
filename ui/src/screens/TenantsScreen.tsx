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
  Alert,
  Button,
  Popconfirm,
  Result,
  Spin,
  Table,
  type TableColumnsType,
  Tag,
  Tooltip,
} from "antd";
import { useCallback, useEffect, useRef, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { PageHeader } from "../components/PageHeader.tsx";
import {
  DeleteTenantDialog,
  TenantDeletionNotice,
  type TenantDeletionResult,
} from "../components/TenantDeletion.tsx";
import { BootstrapTenantAdminDialog, CreateTenantDialog } from "../components/TenantOnboarding.tsx";
import { TENANT_STATUS_META, type TenantListResponse, type TenantRecord } from "../lib/tenants.ts";
import { formatUnixSeconds } from "../lib/tokens.ts";
import type { UserAccount } from "../lib/users.ts";
import type { Notify } from "../types.ts";

/// /tenants — host-admin only (the sidebar hides it for other roles and the
/// server's TenantAdmin guard enforces it). Deletion removes credentials and
/// quarantines data; suspension preserves them. Host-admin has no data access.
export function TenantsScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const [tenants, setTenants] = useState<TenantRecord[]>([]);
  const [openStores, setOpenStores] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [busyName, setBusyName] = useState<string>();
  const [createOpen, setCreateOpen] = useState(false);
  const [bootstrapName, setBootstrapName] = useState<string>();
  const [createdAdmin, setCreatedAdmin] = useState<UserAccount>();
  const onboardingTrigger = useRef<HTMLElement | null>(null);
  const [deleteName, setDeleteName] = useState<string>();
  const [deletionResult, setDeletionResult] = useState<TenantDeletionResult>();
  const deleteTrigger = useRef<HTMLElement | null>(null);
  const refreshButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!deleteName && !loading && deleteTrigger.current) {
      deleteTrigger.current.focus();
      deleteTrigger.current = null;
    }
  }, [deleteName, loading]);

  useEffect(() => {
    if (!createOpen && !bootstrapName && !loading && onboardingTrigger.current) {
      onboardingTrigger.current.focus();
      onboardingTrigger.current = null;
    }
  }, [createOpen, bootstrapName, loading]);

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

  const columns: TableColumnsType<TenantRecord> = [
    {
      title: "Tenant",
      dataIndex: "name",
      render: (value) => <strong>{String(value)}</strong>,
    },
    {
      title: "Status",
      dataIndex: "status",
      width: 100,
      render: (value: TenantRecord["status"]) => {
        const meta = TENANT_STATUS_META[value];
        return <Tag color={meta?.color}>{meta?.label ?? String(value)}</Tag>;
      },
    },
    {
      title: "Store",
      dataIndex: "store_open",
      width: 90,
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
      width: 170,
      render: (value: number) => formatUnixSeconds(value),
    },
    {
      title: "",
      key: "actions",
      width: 280,
      render: (_, tenant) => {
        const busy = busyName !== undefined;
        const spinning = busyName === tenant.name;
        return (
          <span className="actions-row-base tenant-actions">
            <Tooltip title="Available for active tenants without an Admin. Existing administrators manage further accounts from their own session.">
              <Button
                disabled={busy || tenant.status !== "active"}
                size="small"
                type="text"
                onClick={(event) => {
                  onboardingTrigger.current = event.currentTarget;
                  setBootstrapName(tenant.name);
                }}
              >
                Set up admin
              </Button>
            </Tooltip>
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
            <Button
              danger
              disabled={busy}
              icon={<Trash size={14} />}
              onClick={(event) => {
                deleteTrigger.current = event.currentTarget;
                setDeleteName(tenant.name);
              }}
              size="small"
              type="text"
            >
              Delete
            </Button>
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
              onClick={(event) => {
                onboardingTrigger.current = event.currentTarget;
                setCreateOpen(true);
              }}
              type="primary"
            >
              Create tenant
            </Button>
            <Button
              ref={refreshButton}
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
        description="Manage tenant lifecycle. Suspend preserves accounts and data; delete removes credentials and quarantines data. Host-admin cannot read tenant data."
        eyebrow="Access / tenants"
        title="Tenants"
      />
      {deletionResult ? <TenantDeletionNotice result={deletionResult} /> : null}
      {createdAdmin ? (
        <Alert
          className="tenant-onboarding-notice"
          type="success"
          title={`Administrator ${createdAdmin.username} created for ${createdAdmin.tenant}`}
          description="Sign out and sign in as this tenant administrator to manage its users and data. The host-admin session remains unchanged."
        />
      ) : null}
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

      {deleteName ? (
        <DeleteTenantDialog
          api={api}
          name={deleteName}
          onClose={() => setDeleteName(undefined)}
          onDeleted={(result) => {
            deleteTrigger.current = refreshButton.current;
            setDeletionResult(result);
            setDeleteName(undefined);
            void load();
          }}
        />
      ) : null}
      {bootstrapName ? (
        <BootstrapTenantAdminDialog
          api={api}
          name={bootstrapName}
          onClose={() => setBootstrapName(undefined)}
          onCreated={(user) => {
            onboardingTrigger.current = refreshButton.current;
            setCreatedAdmin(user);
            setBootstrapName(undefined);
            void load();
          }}
        />
      ) : null}
      {createOpen ? (
        <CreateTenantDialog
          api={api}
          onClose={() => setCreateOpen(false)}
          onCreated={(name) => {
            setCreateOpen(false);
            setCreatedAdmin(undefined);
            setBootstrapName(name);
            void load();
          }}
        />
      ) : null}
    </main>
  );
}
