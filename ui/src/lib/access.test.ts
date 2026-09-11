import { describe, expect, test } from "bun:test";
import { consoleAccess, landingRoute, routeAccess, type SessionContext } from "./access.ts";

const read = ["documents-read", "graph-read", "search"];
const edit = [...read, "documents-write", "graph-write", "lua-execute"];
const roles: Record<string, string[]> = {
  admin: [...edit, "admin", "promotion-read", "governance-trust"],
  editor: edit,
  viewer: read,
  "script-runner": [...read, "lua-execute"],
  "host-admin": ["tenant-admin"],
  "policy-author": ["promotion-read", "policy-author"],
  "policy-approver": ["promotion-read", "policy-approve"],
  promoter: ["promotion-read", "promotion-decide"],
  "artifact-attestor": ["promotion-read", "artifact-attest"],
};
function context(role: string, edition: SessionContext["edition"]): SessionContext {
  return {
    auth_enabled: true,
    edition,
    scopes: roles[role] ?? [],
    user: { key: "qa", username: "qa-user", role, tenant: "default" },
  };
}

describe("edition and role capability boundaries", () => {
  for (const edition of ["community", "enterprise"] as const) {
    for (const role of Object.keys(roles)) {
      test(`${edition} ${role}: routes, mutations and identity-only landing`, () => {
        const access = consoleAccess(context(role, edition));
        const data = ["admin", "editor", "viewer", "script-runner"].includes(role);
        const writer = ["admin", "editor"].includes(role);
        expect(access.dataWrite).toBe(writer);
        expect(access.graphWrite).toBe(writer);
        expect(routeAccess("/graph", access).allowed).toBe(writer);
        expect(access.luaWrite).toBe(writer);
        expect(routeAccess("/collections/qa?doc=one", access).allowed).toBe(data);
        expect(routeAccess("/query", access).allowed).toBe(data);
        expect(routeAccess("/lua", access).allowed).toBe(writer || role === "script-runner");
        expect(routeAccess("/users/qa", access).allowed).toBe(role === "admin");
        expect(routeAccess("/operations", access).allowed).toBe(role === "admin");
        expect(routeAccess("/review", access).allowed).toBe(edition === "enterprise" && data);
        expect(access.reviewWrite).toBe(edition === "enterprise" && writer);
        expect(access.construct).toBe(edition === "enterprise" && data);
        expect(access.constructWrite).toBe(edition === "enterprise" && writer);
        expect(access.snapshots).toBe(role === "admin");
        expect(routeAccess("/tenants", access).allowed).toBe(
          edition === "enterprise" && role === "host-admin",
        );
        expect(routeAccess(landingRoute(access), access).allowed).toBe(true);
        expect(routeAccess("/overview", access).allowed).toBe(true);
      });
    }
  }
  test("development mode retains data operations and read-only Lua, excluding identity workflows", () => {
    for (const edition of ["community", "enterprise"] as const) {
      const access = consoleAccess({ auth_enabled: false, edition, user: null, scopes: [] });
      expect(access.dataRead && access.dataWrite && access.lua && access.operations).toBe(true);
      expect(access.luaWrite || access.snapshots || access.users || access.tenants).toBe(false);
      expect(access.reviewWrite).toBe(edition === "enterprise");
      expect(routeAccess("/users", access).reason).toContain("authentication");
    }
  });
  test("a role label alone cannot grant actions without server-provided scopes", () => {
    const access = consoleAccess({ ...context("admin", "enterprise"), scopes: [] });
    expect(access.dataWrite || access.operations || access.users || access.snapshots).toBe(false);
    expect(landingRoute(access)).toBe("/overview");
  });
  test("unavailable edition is explained before attempting the route's requests", () => {
    expect(routeAccess("/review", consoleAccess(context("viewer", "community"))).reason).toContain(
      "Enterprise",
    );
  });
});
