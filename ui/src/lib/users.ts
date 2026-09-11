// Domain model + helpers for user management. Shapes mirror
// crates/cognigraph-server/src/routes/users.rs and cognigraph-auth's Role
// (kebab-case serde). The server has create/list/delete only — there is no
// user-update endpoint, so this UI offers none.

export type UserRole = "admin" | "editor" | "viewer" | "script-runner" | "host-admin";

export interface UserAccount {
  key: string;
  username: string;
  role: UserRole;
  tenant: string;
}

export const USER_ROLES: UserRole[] = ["admin", "editor", "viewer", "script-runner", "host-admin"];

/// Labels and scope summaries per role — mirrors Role::scopes() in
/// cognigraph-auth. host-admin deliberately has NO data scopes (D4:
/// it manages tenants, it does not read their graphs).
export const ROLE_META: Record<UserRole, { label: string; color: string; scopes: string }> = {
  admin: {
    label: "Admin",
    color: "geekblue",
    scopes: "Full data access plus user, cache, and snapshot administration.",
  },
  editor: {
    label: "Editor",
    color: "green",
    scopes: "Read/write documents, graph, search, and Lua — no administration.",
  },
  viewer: {
    label: "Viewer",
    color: "default",
    scopes: "Read-only documents, graph, and search.",
  },
  "script-runner": {
    label: "Script runner",
    color: "purple",
    scopes: "Lua execution plus read access — for automation identities.",
  },
  "host-admin": {
    label: "Host admin",
    color: "volcano",
    scopes: "Tenant lifecycle only (create/suspend/delete). No access to any tenant's data.",
  },
};

/// Deleting the account you are signed in with is a foot-gun (the server
/// allows it and revokes its tokens) — the UI refuses it.
export function isProtectedFromDeletion(user: UserAccount, currentUsername?: string): boolean {
  return currentUsername !== undefined && user.username === currentUsername;
}
