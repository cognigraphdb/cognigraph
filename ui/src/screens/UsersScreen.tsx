import { ArrowsClockwise, CircleNotch, Key, UserPlus, UsersThree } from "@phosphor-icons/react";
import { Button, Result, Spin, Table, type TableColumnsType, Tag, Tooltip } from "antd";
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { CreateUserDialog } from "../components/CreateUserDialog.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { ROLE_META, type UserAccount } from "../lib/users.ts";
import type { AuthSession, Notify, ProductEdition } from "../types.ts";

interface UsersScreenProps {
  api: CogniGraphApi;
  notify: Notify;
  session: AuthSession | null;
  edition?: ProductEdition;
}

/// The account list: rows link to /users/{username}, where roles, deletion,
/// and the API-token lifecycle live.
export function UsersScreen({ api, notify, session, edition }: UsersScreenProps) {
  const navigate = useNavigate();
  const [users, setUsers] = useState<UserAccount[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const returnFocus = useRef<HTMLElement | null>(null);
  const refreshButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!createOpen && !loading && returnFocus.current) {
      returnFocus.current.focus();
      returnFocus.current = null;
    }
  }, [createOpen, loading]);
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
      render: (_, user) => (
        <Link
          aria-label={`Open user ${user.username}`}
          className="table-action"
          onClick={(event) => event.stopPropagation()}
          to={`/users/${encodeURIComponent(user.username)}`}
        >
          <strong>{user.username}</strong>
        </Link>
      ),
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
              disabled={loading || !!error || session?.role !== "admin"}
              icon={<UserPlus size={17} />}
              onClick={(event) => {
                returnFocus.current = event.currentTarget;
                setCreateOpen(true);
              }}
              type="primary"
            >
              Create user
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
        description="Tenant administrators create accounts in their own tenant and manage tokens and account deletion. A host administrator provisions a tenant's first admin from Tenants. Existing account roles cannot be edited here."
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

      {createOpen && session?.role === "admin" ? (
        <CreateUserDialog
          api={api}
          notify={notify}
          session={session}
          edition={edition}
          onClose={() => setCreateOpen(false)}
          onCreated={() => {
            returnFocus.current = refreshButton.current;
            setCreateOpen(false);
            void load();
          }}
        />
      ) : null}
    </main>
  );
}
