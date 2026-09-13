import { ArrowsClockwise, CircleNotch, Plus, Prohibit, WarningCircle } from "@phosphor-icons/react";
import { Button, Popconfirm, Spin, Table, type TableColumnsType, Tag } from "antd";
import { useCallback, useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { type ApiTokenRecord, formatUnixSeconds, type TokenGrant } from "../lib/tokens.ts";
import type { Notify } from "../types.ts";
import { CreateTokenDialog } from "./CreateTokenDialog.tsx";
import { TokenGrantDialog } from "./TokenGrantDialog.tsx";

interface UserTokensPanelProps {
  api: CogniGraphApi;
  userKey: string;
  username: string;
  notify: Notify;
}

/// The token list inside an expanded user row: issue, rotate, revoke.
/// Rotation invalidates the old plaintext and yields a new grant, so it
/// funnels through the same shown-once dialog as creation.
export function UserTokensPanel({ api, userKey, username, notify }: UserTokensPanelProps) {
  const [tokens, setTokens] = useState<ApiTokenRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyKey, setBusyKey] = useState<string>();
  const [createOpen, setCreateOpen] = useState(false);
  const [grant, setGrant] = useState<TokenGrant>();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await api.get<ApiTokenRecord[]>(
        `/users/${encodeURIComponent(userKey)}/tokens`,
      );
      setTokens(Array.isArray(response) ? response : []);
    } catch (reason) {
      setTokens([]);
      notify(reason instanceof Error ? reason.message : "Unable to list tokens", "error");
    } finally {
      setLoading(false);
    }
  }, [api, userKey, notify]);

  useEffect(() => void load(), [load]);

  async function revoke(token: ApiTokenRecord) {
    setBusyKey(token.key);
    try {
      await api.delete(
        `/users/${encodeURIComponent(userKey)}/tokens/${encodeURIComponent(token.key)}`,
      );
      notify(`Revoked ${token.name || token.key}`);
      void load();
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Revoke failed", "error");
    } finally {
      setBusyKey(undefined);
    }
  }

  async function rotate(token: ApiTokenRecord) {
    setBusyKey(token.key);
    try {
      const next = await api.post<TokenGrant>(
        `/users/${encodeURIComponent(userKey)}/tokens/${encodeURIComponent(token.key)}/rotate`,
      );
      setGrant(next);
      notify(`Rotated ${token.name || token.key} — the old value no longer works`);
      void load();
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Rotate failed", "error");
    } finally {
      setBusyKey(undefined);
    }
  }

  const now = Math.floor(Date.now() / 1000);
  const columns: TableColumnsType<ApiTokenRecord> = [
    {
      title: "Name",
      dataIndex: "name",
      render: (value) => <strong>{String(value || "unnamed")}</strong>,
    },
    {
      title: "Key",
      dataIndex: "key",
      render: (value) => <span className="mono-cell">{String(value)}</span>,
    },
    {
      title: "Created",
      dataIndex: "created_at",
      width: 180,
      render: (value: number) => formatUnixSeconds(value),
    },
    {
      title: "Expires",
      dataIndex: "expires_at",
      width: 200,
      render: (value: number | null) =>
        value ? (
          value <= now ? (
            <Tag color="red">Expired</Tag>
          ) : (
            formatUnixSeconds(value)
          )
        ) : (
          <Tag>Never</Tag>
        ),
    },
    {
      title: "",
      key: "actions",
      width: 190,
      render: (_, token) => (
        <span className="actions-row-base token-actions">
          <Popconfirm
            cancelText="Cancel"
            icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
            okText="Rotate"
            onConfirm={() => void rotate(token)}
            placement="topLeft"
            title="Rotate this token? The current value stops working immediately."
          >
            {/* icon swap instead of the antd `loading` prop: combining
                `icon` with `loading` routes the custom icon through
                @ant-design/icons and trips its dev warning. */}
            <Button
              disabled={busyKey !== undefined}
              icon={
                busyKey === token.key ? (
                  <CircleNotch className="cg-spin" weight="bold" />
                ) : (
                  <ArrowsClockwise size={14} />
                )
              }
              size="small"
              type="text"
            >
              Rotate
            </Button>
          </Popconfirm>
          <Popconfirm
            cancelText="Cancel"
            icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
            okText="Revoke"
            okButtonProps={{ danger: true }}
            onConfirm={() => void revoke(token)}
            placement="topLeft"
            title="Revoke this token? Anything using it loses access immediately."
          >
            <Button
              danger
              disabled={busyKey !== undefined}
              icon={<Prohibit size={14} />}
              size="small"
              type="text"
            >
              Revoke
            </Button>
          </Popconfirm>
        </span>
      ),
    },
  ];

  return (
    <div className="token-panel">
      <div className="workspace-controls token-panel-controls">
        <Button icon={<Plus size={15} />} onClick={() => setCreateOpen(true)} size="small">
          New token
        </Button>
        <span className="workspace-count">
          {tokens.length} {tokens.length === 1 ? "token" : "tokens"}
        </span>
      </div>
      <Spin spinning={loading}>
        <Table<ApiTokenRecord>
          columns={columns}
          dataSource={tokens}
          locale={{ emptyText: "No API tokens issued for this account" }}
          pagination={false}
          rowKey="key"
          size="small"
        />
      </Spin>

      {createOpen ? (
        <CreateTokenDialog
          api={api}
          onClose={() => setCreateOpen(false)}
          onIssued={(issued) => {
            setCreateOpen(false);
            setGrant(issued);
            void load();
          }}
          userKey={userKey}
          username={username}
        />
      ) : null}
      {grant ? (
        <TokenGrantDialog grant={grant} onClose={() => setGrant(undefined)} username={username} />
      ) : null}
    </div>
  );
}
