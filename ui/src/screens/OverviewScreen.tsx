import {
  ArrowsClockwise,
  CircleNotch,
  Database,
  Files,
  Gauge,
  HardDrives,
  Pulse,
} from "@phosphor-icons/react";
import { Alert, Button, Table, type TableColumnsType, Tag, Tooltip } from "antd";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import {
  COLLECTION_TYPE_META,
  type CollectionInfo,
  type CollectionListResponse,
  isBrowsable,
} from "../lib/collections.ts";
import { ROLE_META, type UserRole } from "../lib/users.ts";
import type { AuthSession, HealthSnapshot, JsonObject } from "../types.ts";

interface OverviewScreenProps {
  api: CogniGraphApi;
  health: HealthSnapshot;
  session: AuthSession | null;
  onRefresh: () => void;
}

/// The signed-in tenant's dashboard: service readiness plus THIS tenant's
/// data at a glance. There are no connection settings — the server URL
/// derives from the load origin and the session comes from the login.
export function OverviewScreen({ api, health, session, onRefresh }: OverviewScreenProps) {
  const { dataRead, dataWrite, operations, context } = useAccess();
  const navigate = useNavigate();
  const [cache, setCache] = useState<JsonObject>();
  const [collections, setCollections] = useState<CollectionInfo[]>([]);
  const [catalogError, setCatalogError] = useState("");

  useEffect(() => {
    if (health.status !== "online") return;
    if (operations)
      api
        .get<JsonObject>("/cache/stats")
        .then(setCache)
        .catch(() => setCache(undefined));
    if (dataRead)
      api
        .get<CollectionListResponse>("/collections")
        .then((response) => {
          setCollections(response.collections ?? []);
          setCatalogError("");
        })
        .catch((reason: Error) => {
          setCollections([]);
          setCatalogError(reason.message);
        });
  }, [api, health.status, dataRead, operations]);

  const totals = cache?.totals as JsonObject | undefined;
  const documentCount = collections.reduce((sum, info) => sum + info.count, 0);
  const role = session ? ROLE_META[session.role as UserRole] : undefined;

  const columns: TableColumnsType<CollectionInfo> = [
    {
      title: "Collection",
      dataIndex: "name",
      render: (value) => <strong className="mono-cell">{String(value)}</strong>,
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
  ];

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <Button
            disabled={health.status === "checking"}
            icon={
              health.status === "checking" ? (
                <CircleNotch className="cg-spin" weight="bold" />
              ) : (
                <ArrowsClockwise aria-hidden="true" size={17} />
              )
            }
            onClick={onRefresh}
          >
            {health.status === "checking" ? "Refreshing…" : "Refresh"}
          </Button>
        }
        description={
          session && dataRead
            ? `Everything on this page is scoped to the ${session.tenant} tenant — the workspace your account belongs to.`
            : "Verified identity and live service readiness from the running CogniGraph API."
        }
        eyebrow="System / overview"
        title="Overview"
      />

      {session ? (
        <dl className="user-facts overview-session">
          <dt>Signed in as</dt>
          <dd>
            <strong>{session.username}</strong>
          </dd>
          <dt>Role</dt>
          <dd>
            {role ? (
              <Tooltip title={role.scopes}>
                <Tag color={role.color}>{role.label}</Tag>
              </Tooltip>
            ) : (
              <Tag>{session.role}</Tag>
            )}
          </dd>
          <dt>Tenant</dt>
          <dd className="mono-cell">{session.tenant}</dd>
        </dl>
      ) : null}

      {!context.auth_enabled ? (
        <Alert
          type="warning"
          showIcon
          title="Authentication is disabled"
          description="Development mode permits data reads and writes and read-only Lua. User and tenant administration, snapshots and signed governance require authenticated accounts."
        />
      ) : null}
      {!dataRead ? (
        <Alert
          type="info"
          showIcon
          title="This account has no tenant data access"
          description={
            context.user?.role === "host-admin"
              ? context.edition === "enterprise"
                ? "Use Tenants to manage tenant lifecycle. Host administrators cannot read tenant data."
                : "Tenant lifecycle requires an Enterprise server. This Community console can show your identity and service status."
              : context.edition === "enterprise"
                ? "Your governance role is verified. Signed governance workflows are available through the API; the console does not yet provide those workflows. Review and Construct operate on tenant data and require separate data scopes."
                : "Governance workflows require an Enterprise server. This Community console can show your identity and service status."
          }
        />
      ) : null}
      <section className="metric-grid" aria-label="System status">
        <Metric
          icon={Pulse}
          label="Service"
          value={health.status}
          detail={health.service ?? health.error}
        />
        <Metric
          icon={Database}
          label="Database"
          value={health.database ?? "unknown"}
          detail="Backend ping"
        />
        {dataRead ? (
          <Metric
            icon={Files}
            label="Tenant data"
            value={documentCount.toLocaleString()}
            detail={`${collections.length} ${collections.length === 1 ? "collection" : "collections"}`}
          />
        ) : null}
        <Metric
          icon={Gauge}
          label="Latency"
          value={`${health.latencyMs ?? 0} ms`}
          detail="Two health checks"
        />
        {operations ? (
          <Metric
            icon={HardDrives}
            label="Query cache"
            value={cache?.enabled ? `${cache.entries ?? 0} entries` : "disabled"}
            detail={
              totals
                ? `${Math.round(Number(totals.hit_rate ?? 0) * 100)}% hit rate`
                : "Server setting"
            }
          />
        ) : null}
      </section>

      {dataRead ? (
        <section className="user-list" aria-label="Tenant collections">
          <div className="workspace-controls">
            <h2 className="overview-subheading">Collections in this tenant</h2>
            <span className="workspace-count">
              {catalogError ? catalogError : `${documentCount.toLocaleString()} entries total`}
            </span>
          </div>
          <Table<CollectionInfo>
            columns={columns}
            dataSource={collections}
            locale={{
              emptyText: catalogError
                ? "The collection catalog is unavailable for this role."
                : dataWrite
                  ? "No collections yet — create a document to start one."
                  : "No collections in this tenant.",
            }}
            onRow={(info) => ({
              onClick: () => {
                if (isBrowsable(info)) navigate(`/collections/${encodeURIComponent(info.name)}`);
              },
            })}
            pagination={false}
            rowClassName={(info) => (isBrowsable(info) ? "clickable-row" : "")}
            rowKey="name"
            size="small"
          />
        </section>
      ) : null}
    </main>
  );
}

interface MetricProps {
  icon: typeof Pulse;
  label: string;
  value: string;
  detail?: string;
}

function Metric({ icon: Icon, label, value, detail }: MetricProps) {
  return (
    <article className="metric-card">
      <Icon aria-hidden="true" size={22} />
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
