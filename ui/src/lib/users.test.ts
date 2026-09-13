import { describe, expect, test } from "bun:test";
import type { UserAccount } from "./users.ts";
import {
  creatableUserRoles,
  isProtectedFromDeletion,
  ROLE_META,
  tenantUserRequest,
  USER_ROLES,
} from "./users.ts";

describe("tenant-local provisioning", () => {
  const actor = { username: "qa-admin", role: "admin", tenant: "qa-tenant" };
  test("only tenant administrators can create accounts; host-admin is never a creatable role", () => {
    for (const role of USER_ROLES.filter((role) => role !== "admin")) {
      expect(creatableUserRoles(role, "enterprise")).toEqual([]);
      expect(() =>
        tenantUserRequest(
          { username: "new", password: "synthetic", role: "viewer" },
          { ...actor, role },
          "enterprise",
        ),
      ).toThrow();
    }
    expect(creatableUserRoles(undefined, "enterprise")).toEqual([]);
    expect(() =>
      tenantUserRequest(
        { username: "new", password: "synthetic", role: "host-admin" },
        actor,
        "enterprise",
      ),
    ).toThrow();
  });
  test("governance choices require a confirmed Enterprise edition", () => {
    for (const edition of [undefined, "community"] as const) {
      expect(creatableUserRoles("admin", edition)).toEqual([
        "admin",
        "editor",
        "viewer",
        "script-runner",
      ]);
      expect(() =>
        tenantUserRequest(
          { username: "new", password: "synthetic", role: "policy-author" },
          actor,
          edition,
        ),
      ).toThrow();
    }
    expect(creatableUserRoles("admin", "enterprise")).toEqual([
      "admin",
      "editor",
      "viewer",
      "script-runner",
      "policy-author",
      "policy-approver",
      "promoter",
      "artifact-attestor",
    ]);
  });
  test("the payload cannot select another tenant and preserves the entered password", () => {
    const input = {
      username: " new-user ",
      password: " keep spaces ",
      role: "viewer" as const,
      tenant: "another-tenant",
    };
    expect(tenantUserRequest(input, actor, "enterprise")).toEqual({
      username: "new-user",
      password: " keep spaces ",
      role: "viewer",
    });
  });
});

const account = (username: string): UserAccount => ({
  key: "k1",
  username,
  role: "editor",
  tenant: "default",
});

describe("isProtectedFromDeletion", () => {
  test("the signed-in account cannot be deleted", () => {
    expect(isProtectedFromDeletion(account("admin"), "admin")).toBe(true);
  });

  test("other accounts can be deleted", () => {
    expect(isProtectedFromDeletion(account("vera"), "admin")).toBe(false);
  });

  test("unknown session protects nothing", () => {
    expect(isProtectedFromDeletion(account("admin"), undefined)).toBe(false);
  });
});

describe("ROLE_META", () => {
  test("covers every role", () => {
    for (const role of USER_ROLES) {
      expect(ROLE_META[role].label.length).toBeGreaterThan(0);
      expect(ROLE_META[role].scopes.length).toBeGreaterThan(0);
    }
  });
});
