import { ArrowsClockwise, CircleNotch, Key, UserPlus, UsersThree } from "@phosphor-icons/react";
import { Button, Result, Spin, Table, type TableColumnsType, Tag, Tooltip } from "antd";
import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { CreateUserDialog } from "../components/CreateUserDialog.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { ROLE_META, type UserAccount } from "../lib/users.ts";
import type { Notify } from "../types.ts";

interface UsersScreenProps {
  api: CogniGraphApi;
  notify: Notify;
}

/// The account list: rows link to /users/{username}, where roles, deletion,
/// and the API-token lifecycle live.
export function UsersScreen({ api, notify }: UsersScreenProps) {
  const navigate = useNavigate();
  const [users, setUsers] = useState<UserAccount[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const friendlyError = error.includes("auth is disabled")
    ? "Authentication is disabled on this server."
    : error;

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await api.get<unknown>("/users");
      setUsers(Array.isArray(response) ? (response as UserAccount[]) : []);
    } catch (reason) {
      setUsers([]);
      setError(reason instanceof Error ? reason.message : "Unable to list users");
    } finally {
      setLoading(false);
    }
  }, [api]);

  useEffect(() => void load(), [load]);

  const columns: TableColumnsType<UserAccount> = [
    {
      title: "Username",
      dataIndex: "username",
      render: (value) => <strong>{String(value ?? "—")}</strong>,
    },
    {
      title: "Role",
      dataIndex: "role",
      width: 150,
      render: (value: UserAccount["role"]) => {
        const meta = ROLE_META[value];
        return meta ? (
          <Tooltip title={meta.scopes}>
            <Tag color={meta.color}>{meta.label}</Tag>
          </Tooltip>
        ) : (
          <Tag>{String(value ?? "—")}</Tag>
        );
      },
    },
    {
      title: "Tenant",
      dataIndex: "tenant",
      width: 130,
      render: (value) => String(value ?? "default"),
    },
    {
      title: "Key",
      dataIndex: "key",
      render: (value) => (
        <span className="mono-cell">
          <Key aria-hidden="true" size={14} /> {String(value ?? "—")}
        </span>
      ),
    },
  ];

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <>
            <Button
              disabled={loading || !!error}
              icon={<UserPlus size={17} />}
              onClick={() => setCreateOpen(true)}
              type="primary"
            >
              Create user
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
        description="Create accounts and open one to manage its role, tokens, and lifecycle. Each account belongs to one tenant, which scopes everything it sees; tenant workspaces themselves are managed by a host-admin session."
        eyebrow="Access / users"
        title="Users"
      />
      <section className="user-list">
        <div className="workspace-controls">
          <span className="workspace-count">
            {users.length} {users.length === 1 ? "user" : "users"}
          </span>
        </div>
        <Spin
          description="Loading users…"
          indicator={<CircleNotch className="cg-spin" weight="bold" />}
          spinning={loading}
        >
          {!loading && error ? (
            <Result
              icon={<UsersThree aria-hidden="true" size={34} />}
              status="info"
              subTitle={
                <>
                  {friendlyError}
                  <br />
                  <small>Enable COGNIGRAPH_AUTH_ENABLED and sign in as an admin.</small>
                </>
              }
              title="User management is unavailable"
            />
          ) : null}
          {!error ? (
            <Table<UserAccount>
              columns={columns}
              dataSource={users}
              locale={{ emptyText: "No users configured" }}
              onRow={(user) => ({
                onClick: () => navigate(`/users/${encodeURIComponent(user.username)}`),
              })}
              pagination={false}
              rowClassName={() => "clickable-row"}
              rowKey={(user) => user.key ?? user.username}
              size="small"
            />
          ) : null}
        </Spin>
      </section>

      {createOpen ? (
        <CreateUserDialog
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
