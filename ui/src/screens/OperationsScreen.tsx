import {
  ArrowClockwise,
  ArrowSquareOut,
  Broom,
  ChartLineUp,
  CheckCircle,
  CircleNotch,
  DownloadSimple,
  FileCode,
  Timer,
  UploadSimple,
  WarningCircle,
  XCircle,
} from "@phosphor-icons/react";
import { App as AntApp, Button, Table, type TableColumnsType, Tag, Tooltip } from "antd";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { ApiError, type CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { formatUptime, parseMetrics, type ServerMetrics } from "../lib/metrics.ts";
import {
  formatEventTime,
  type LogEvent,
  normalizeLogEvent,
  statusClass,
} from "../lib/server-logs.ts";
import type { JsonObject, Notify } from "../types.ts";

export function OperationsScreen({
  api,
  baseUrl,
  notify,
}: {
  api: CogniGraphApi;
  baseUrl: string;
  notify: Notify;
}) {
  const { snapshots } = useAccess();
  const [result, setResult] = useState<unknown>();
  const [loading, setLoading] = useState<"cache" | "clear" | "export">();
  const [metrics, setMetrics] = useState<ServerMetrics>();
  const [metricsError, setMetricsError] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [logs, setLogs] = useState<LogEvent[]>();
  const [logsState, setLogsState] = useState<"ok" | "forbidden" | "error">("ok");
  const { modal } = AntApp.useApp();

  const refresh = useCallback(async () => {
    setRefreshing(true);
    const metricsCall = api
      .metricsText()
      .then((text) => {
        setMetrics(parseMetrics(text));
        setMetricsError(false);
      })
      .catch(() => setMetricsError(true));
    const logsCall = api
      .get<{ events: JsonObject[] }>("/admin/logs")
      .then(({ events }) => {
        setLogs(events.map(normalizeLogEvent));
        setLogsState("ok");
      })
      .catch((error: unknown) => {
        // A non-admin session lacks the Admin scope: an honest gated
        // state, not an error toast.
        setLogsState(error instanceof ApiError && error.status === 403 ? "forbidden" : "error");
        setLogs(undefined);
      });
    await Promise.all([metricsCall, logsCall]);
    setRefreshing(false);
  }, [api]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const cacheStats = async () => {
    setLoading("cache");
    try {
      setResult(await api.get<JsonObject>("/cache/stats"));
    } catch (error) {
      setResult({ error: error instanceof Error ? error.message : "Cache request failed" });
    } finally {
      setLoading(undefined);
    }
  };

  const clearCache = async () => {
    setLoading("clear");
    try {
      const response = await api.post<JsonObject>("/cache/clear");
      setResult(response);
      notify("Query cache cleared");
    } catch (error) {
      setResult({ error: error instanceof Error ? error.message : "Cache clear failed" });
      notify(error instanceof Error ? error.message : "Cache clear failed", "error");
    } finally {
      setLoading(undefined);
    }
  };

  const confirmCacheClear = () => {
    modal.confirm({
      centered: true,
      icon: <WarningCircle color="var(--warning)" size={22} weight="fill" />,
      title: "Clear query cache?",
      content: "All cached query results for the active tenant will be removed.",
      okText: "Clear cache",
      okButtonProps: { danger: true },
      onOk: () => void clearCache(),
    });
  };

  const exportSnapshot = async () => {
    if (!snapshots) return;
    setLoading("export");
    try {
      const snapshot = await api.get<JsonObject>("/admin/export");
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(snapshot, null, 2)], { type: "application/json" }),
      );
      const link = document.createElement("a");
      link.href = url;
      link.download = `cognigraph-snapshot-${new Date().toISOString().slice(0, 10)}.json`;
      link.click();
      URL.revokeObjectURL(url);
      setResult({
        exported: true,
        collections: Object.keys((snapshot.collections as JsonObject) ?? {}).length,
      });
      notify("Snapshot export prepared");
    } catch (error) {
      setResult({ error: error instanceof Error ? error.message : "Export failed" });
      notify(error instanceof Error ? error.message : "Export failed", "error");
    } finally {
      setLoading(undefined);
    }
  };

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <Button
            icon={
              refreshing ? (
                <CircleNotch className="cg-spin" size={16} />
              ) : (
                <ArrowClockwise size={16} />
              )
            }
            onClick={() => void refresh()}
          >
            Refresh
          </Button>
        }
        description="Run safe operational checks and administrative actions exposed by the server."
        eyebrow="System / operations"
        title="Operations"
      />
      <section className="metric-grid" aria-label="Server metrics">
        <Metric
          icon={ChartLineUp}
          label="Requests served"
          value={metricsError ? "—" : (metrics?.requests.toLocaleString() ?? "…")}
          detail="since process start"
        />
        <Metric
          icon={CheckCircle}
          label="Successful (2xx–3xx)"
          value={metricsError ? "—" : (metrics?.success.toLocaleString() ?? "…")}
          detail="2xx and 3xx responses"
        />
        <Metric
          icon={XCircle}
          label="Errors"
          value={
            metricsError || !metrics
              ? "—"
              : `${metrics.clientErrors.toLocaleString()} · ${metrics.serverErrors.toLocaleString()}`
          }
          detail="4xx client · 5xx server"
        />
        <Metric
          icon={Timer}
          label="Mean latency"
          value={
            metricsError
              ? "—"
              : metrics?.avgLatencyMs !== undefined
                ? `${metrics.avgLatencyMs.toFixed(1)} ms`
                : "…"
          }
          detail="all requests, mean"
        />
        <Metric
          icon={ArrowClockwise}
          label="Uptime"
          value={metricsError ? "—" : metrics ? formatUptime(metrics.uptimeSeconds) : "…"}
          detail={metricsError ? "metrics unavailable" : "current process"}
        />
      </section>
      <div className="operations-grid">
        <section className="console-card action-list">
          <Operation
            icon={Broom}
            title="Query cache"
            detail="Inspect cache behavior or clear all entries."
          >
            <Button disabled={Boolean(loading)} onClick={cacheStats}>
              {loading === "cache" ? <CircleNotch className="cg-spin" /> : null}
              {loading === "cache" ? "Loading…" : "View stats"}
            </Button>
            <Button danger disabled={Boolean(loading)} onClick={confirmCacheClear}>
              {loading === "clear" ? <CircleNotch className="cg-spin" /> : null}
              {loading === "clear" ? "Clearing…" : "Clear cache"}
            </Button>
          </Operation>
          <Operation
            icon={DownloadSimple}
            title="Snapshot export"
            detail={
              snapshots
                ? "Download a hot JSON backup from the active tenant."
                : "Snapshot export requires authentication and an Admin account."
            }
          >
            <Button
              icon={
                loading === "export" ? (
                  <CircleNotch className="cg-spin" />
                ) : (
                  <DownloadSimple size={16} />
                )
              }
              disabled={!snapshots || Boolean(loading)}
              onClick={exportSnapshot}
            >
              {loading === "export" ? "Exporting…" : "Export"}
            </Button>
          </Operation>
          <Operation
            icon={UploadSimple}
            title="Snapshot import"
            detail="Restore is deliberately gated until a file-review flow is approved."
          >
            <Tooltip title="Import requires a reviewed file-preview flow">
              <Button disabled>Choose snapshot</Button>
            </Tooltip>
          </Operation>
          <Operation
            icon={FileCode}
            title="API contract"
            detail="Open the server-owned OpenAPI specification."
          >
            <Button
              href={`${baseUrl}/openapi.yaml`}
              icon={<ArrowSquareOut size={16} />}
              rel="noreferrer"
              target="_blank"
            >
              Open spec
            </Button>
          </Operation>
        </section>
        <section className="console-card result-panel operation-result">
          <div className="card-heading compact-heading">
            <h2>Operation result</h2>
          </div>
          <div className="result-body">
            <JsonResult value={result} />
          </div>
        </section>
      </div>
      <ServerLogs logs={logs} state={logsState} />
    </main>
  );
}

function ServerLogs({ logs, state }: { logs?: LogEvent[]; state: "ok" | "forbidden" | "error" }) {
  const columns: TableColumnsType<LogEvent> = [
    { title: "Time", key: "time", width: 96, render: (_, e) => formatEventTime(e.at) },
    { title: "Method", dataIndex: "method", width: 74 },
    {
      title: "Status",
      key: "status",
      width: 78,
      render: (_, e) => (
        <Tag color={statusClass(e.status) === "server" ? "error" : "warning"}>{e.status}</Tag>
      ),
    },
    {
      title: "Path",
      dataIndex: "path",
      render: (path: string) => <span className="mono-cell">{path}</span>,
    },
    { title: "Message", dataIndex: "message", ellipsis: true },
    { title: "Latency", key: "latency", width: 82, render: (_, e) => `${e.latencyMs} ms` },
  ];

  return (
    <section className="server-logs">
      <div className="workspace-controls">
        <h2 className="server-logs-title">Recent errors</h2>
        <span className="workspace-count">non-2xx responses, newest first</span>
      </div>
      {state === "forbidden" ? (
        <p className="server-logs-note">
          The server error log requires an admin session — your role can’t read it.
        </p>
      ) : state === "error" ? (
        <p className="server-logs-note">Couldn’t load the server error log.</p>
      ) : (
        <Table<LogEvent>
          columns={columns}
          dataSource={logs}
          loading={logs === undefined}
          locale={{ emptyText: "No errors recorded since the server started." }}
          pagination={false}
          rowKey="seq"
          scroll={{ y: 320 }}
          size="small"
        />
      )}
    </section>
  );
}

function Operation({
  icon: Icon,
  title,
  detail,
  children,
}: {
  icon: typeof Broom;
  title: string;
  detail: string;
  children: ReactNode;
}) {
  return (
    <article className="operation-row">
      <div className="operation-icon">
        <Icon aria-hidden="true" size={21} />
      </div>
      <div>
        <strong>{title}</strong>
        <p>{detail}</p>
      </div>
      <div className="actions-row-base operation-actions">{children}</div>
    </article>
  );
}

function Metric({
  icon: Icon,
  label,
  value,
  detail,
}: {
  icon: typeof Broom;
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <article className="metric-card metric-card-literal">
      <Icon aria-hidden="true" size={22} />
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
