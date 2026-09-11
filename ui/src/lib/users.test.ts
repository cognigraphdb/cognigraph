import { describe, expect, test } from "bun:test";
import type { UserAccount } from "./users.ts";
import { isProtectedFromDeletion, ROLE_META, USER_ROLES } from "./users.ts";

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
