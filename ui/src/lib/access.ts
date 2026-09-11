import type { AuthSession, ProductEdition } from "../types.ts";

export interface SessionContext {
  auth_enabled: boolean;
  user: (AuthSession & { key: string }) | null;
  scopes: string[];
  edition: ProductEdition;
}

export function parseSessionContext(value: unknown): SessionContext {
  const invalid = () => new Error("The server did not return a valid CogniGraph session response.");
  if (!value || typeof value !== "object" || Array.isArray(value)) throw invalid();
  const context = value as SessionContext;
  if (
    typeof context.auth_enabled !== "boolean" ||
    !["community", "enterprise"].includes(context.edition) ||
    !Array.isArray(context.scopes) ||
    !context.scopes.every((scope) => typeof scope === "string")
  )
    throw invalid();
  if (context.auth_enabled) {
    if (
      !context.user ||
      ["key", "username", "role", "tenant"].some(
        (key) =>
          typeof context.user?.[key as keyof typeof context.user] !== "string" ||
          !context.user[key as keyof typeof context.user],
      )
    )
      throw invalid();
  } else if (context.user !== null || context.scopes.length !== 0) throw invalid();
  return context;
}

// Mirrors existing auth-disabled development behavior, not an invented Admin.
// User/tenant administration, snapshots and signed governance still need identity.
const developmentScopes = [
  "documents-read",
  "documents-write",
  "graph-read",
  "graph-write",
  "search",
  "lua-execute",
  "admin",
];

export function consoleAccess(context: SessionContext) {
  const scopes = context.auth_enabled ? context.scopes : developmentScopes;
  const has = (scope: string) => scopes.includes(scope);
  const enterprise = context.edition === "enterprise";
  return {
    context,
    dataRead: has("documents-read"),
    dataWrite: has("documents-write"),
    // The current POST /graph/traverse route is guarded by GraphWrite.
    graphExplore: has("graph-write"),
    graphWrite: has("graph-write"),
    search: has("search"),
    lua: has("lua-execute"),
    luaWrite: context.auth_enabled && has("documents-write"),
    review: enterprise && has("graph-read"),
    reviewWrite: enterprise && has("graph-write"),
    construct: enterprise && has("graph-read"),
    constructWrite: enterprise && has("graph-write"),
    operations: has("admin"),
    users: context.auth_enabled && has("admin"),
    snapshots: context.auth_enabled && context.user?.role === "admin" && has("admin"),
    tenants: enterprise && context.auth_enabled && has("tenant-admin"),
  };
}

export type ConsoleAccess = ReturnType<typeof consoleAccess>;

export function routeAccess(
  path: string,
  access: ConsoleAccess,
): { allowed: boolean; reason?: string } {
  const page = path.split("/")[1];
  if (!page || page === "overview") return { allowed: true };
  if (
    ["review", "construct", "tenants"].includes(page) &&
    access.context.edition !== "enterprise"
  ) {
    return { allowed: false, reason: "This page requires an Enterprise server." };
  }
  if (["users", "tenants"].includes(page) && !access.context.auth_enabled) {
    return {
      allowed: false,
      reason: "This page requires authentication to be enabled and an authorized account.",
    };
  }
  if (page === "graph" && !access.graphExplore) {
    return {
      allowed: false,
      reason:
        "Graph exploration currently requires graph-write scope on the server. Read-only graph queries remain available through Query.",
    };
  }
  const pages: Record<string, boolean> = {
    collections: access.dataRead,
    query: access.search,
    graph: access.graphExplore,
    review: access.review,
    construct: access.construct,
    lua: access.lua,
    operations: access.operations,
    users: access.users,
    tenants: access.tenants,
  };
  return pages[page] === true
    ? { allowed: true }
    : { allowed: false, reason: "This page is unavailable for your current role." };
}

export function landingRoute(access: ConsoleAccess): string {
  return access.tenants ? "/tenants" : access.dataRead ? "/collections" : "/overview";
}
