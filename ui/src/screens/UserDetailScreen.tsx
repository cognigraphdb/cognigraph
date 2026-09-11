import {
  ArrowsClockwise,
  CircleNotch,
  Trash,
  UsersThree,
  WarningCircle,
} from "@phosphor-icons/react";
import { Button, Popconfirm, Spin, Tag, Tooltip } from "antd";
import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { PageHeader } from "../components/PageHeader.tsx";
import { UserTokensPanel } from "../components/UserTokensPanel.tsx";
import { isProtectedFromDeletion, ROLE_META, type UserAccount } from "../lib/users.ts";
import type { Notify } from "../types.ts";

interface UserDetailScreenProps {
  api: CogniGraphApi;
  notify: Notify;
  currentUsername?: string;
}

/// /users/:username — one account's roles and API tokens. The server has
/// no fetch-by-key endpoint, so the account is resolved from the list.
export function UserDetailScreen({ api, notify, currentUsername }: UserDetailScreenProps) {
  const { username = "" } = useParams();
  const navigate = useNavigate();
  const [user, setUser] = useState<UserAccount>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await api.get<UserAccount[]>("/users");
      const found = (Array.isArray(response) ? response : []).find(
        (account) => account.username === username,
      );
      setUser(found);
    } catch (reason) {
      setUser(undefined);
      setError(reason instanceof Error ? reason.message : "Unable to load the account");
    } finally {
      setLoading(false);
    }
  }, [api, username]);

  useEffect(() => void load(), [load]);

  async function removeUser(account: UserAccount) {
    setDeleting(true);
    try {
      // The server cascades: the user's API tokens are revoked with it.
      await api.delete(`/users/${encodeURIComponent(account.key)}`);
      notify(`Deleted ${account.username} and revoked their tokens`);
      navigate("/users");
    } catch (reason) {
      notify(reason instanceof Error ? reason.message : "Delete failed", "error");
      setDeleting(false);
    }
  }

  if (loading) {
    return (
      <main className="page-workspace">
        <Spin
          className="panel-loading"
          indicator={<CircleNotch className="cg-spin" weight="bold" />}
        />
      </main>
    );
  }

  if (error || !user) {
    return (
      <main className="page-workspace">
        <div className="empty-state-base user-missing">
          <UsersThree aria-hidden="true" size={34} />
          <span>{error || `No account named “${username}”.`}</span>
          <Button onClick={() => navigate("/users")}>Back to users</Button>
        </div>
      </main>
    );
  }

  const role = ROLE_META[user.role];
  const isSelf = isProtectedFromDeletion(user, currentUsername);
  // Icon swap instead of the antd `loading` prop: `icon` + `loading`
  // together route the custom icon through @ant-design/icons and trip
  // its dev warning.
  const deleteButton = (
    <Button
      danger
      disabled={isSelf || deleting}
      icon={deleting ? <CircleNotch className="cg-spin" weight="bold" /> : <Trash size={16} />}
    >
      Delete user
    </Button>
  );

  return (
    <main className="page-workspace">
      <PageHeader
        actions={
          <>
            {isSelf ? (
              <Tooltip title="You cannot delete the account you are signed in with.">
                {deleteButton}
              </Tooltip>
            ) : (
              <Popconfirm
                cancelText="Cancel"
                icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
                okButtonProps={{ danger: true }}
                okText="Delete"
                onConfirm={() => void removeUser(user)}
                placement="bottomRight"
                title={`Delete ${user.username}? Their API tokens are revoked with them.`}
              >
                {deleteButton}
              </Popconfirm>
            )}
            <Button icon={<ArrowsClockwise size={17} />} onClick={() => void load()}>
              Refresh
            </Button>
          </>
        }
        description={role?.scopes ?? user.role}
        eyebrow={`Access / users / ${user.username}`}
        title={user.username}
      />

      <dl className="user-facts">
        <dt>Role</dt>
        <dd>
          <Tag color={role?.color}>{role?.label ?? user.role}</Tag>
        </dd>
        <dt>Tenant</dt>
        <dd>{user.tenant}</dd>
        <dt>Key</dt>
        <dd className="mono-cell">{user.key}</dd>
      </dl>

      <section className="user-tokens-section">
        <h2>API tokens</h2>
        <UserTokensPanel api={api} notify={notify} userKey={user.key} username={user.username} />
      </section>
    </main>
  );
}
