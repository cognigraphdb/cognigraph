// Domain model + helpers for user management. Shapes mirror
// crates/cognigraph-server/src/routes/users.rs and cognigraph-auth's Role
// (kebab-case serde). The server has create/list/delete only — there is no
// user-update endpoint, so this UI offers none.

import type { AuthSession, ProductEdition } from "../types.ts";

const BASIC_ROLES = ["admin", "editor", "viewer", "script-runner"] as const;
const GOVERNANCE_ROLES = [
  "policy-author",
  "policy-approver",
  "promoter",
  "artifact-attestor",
] as const;
export const USER_ROLES = [...BASIC_ROLES, ...GOVERNANCE_ROLES, "host-admin"] as const;
export type UserRole = (typeof USER_ROLES)[number];

export interface UserAccount {
  key: string;
  username: string;
  role: UserRole;
  tenant: string;
}

export function creatableUserRoles(actorRole?: string, edition?: ProductEdition): UserRole[] {
  if (actorRole !== "admin") return [];
  return edition === "enterprise" ? [...BASIC_ROLES, ...GOVERNANCE_ROLES] : [...BASIC_ROLES];
}

export function tenantUserRequest(
  values: { username: string; password: string; role: UserRole },
  actor: AuthSession,
  edition?: ProductEdition,
) {
  if (!creatableUserRoles(actor.role, edition).includes(values.role)) {
    throw new Error("This role is not available for user creation in the current session.");
  }
  // Omit tenant entirely: the server derives it from the authenticated Admin.
  return { username: values.username.trim(), password: values.password, role: values.role };
}

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
  "policy-author": {
    label: "Policy author",
    color: "cyan",
    scopes: "Read promotion evidence and author policy. Cannot approve or promote; no data access.",
  },
  "policy-approver": {
    label: "Policy approver",
    color: "blue",
    scopes:
      "Read promotion evidence and independently approve policy. Cannot author or promote; no data access.",
  },
  promoter: {
    label: "Promoter",
    color: "gold",
    scopes: "Read promotion evidence and decide promotions under approved policy. No data access.",
  },
  "artifact-attestor": {
    label: "Artifact attestor",
    color: "magenta",
    scopes:
      "Read promotion evidence and attest external artifacts. No policy, promotion or data authority.",
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
