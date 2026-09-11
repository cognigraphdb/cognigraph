import { describe, expect, test } from "bun:test";
import { canManageTenants, TENANT_STATUS_META } from "./tenants.ts";

describe("canManageTenants", () => {
  test("only host-admin sessions manage tenants", () => {
    expect(canManageTenants("host-admin")).toBe(true);
    for (const role of ["admin", "editor", "viewer", "script-runner", undefined]) {
      expect(canManageTenants(role)).toBe(false);
    }
  });
});

describe("TENANT_STATUS_META", () => {
  test("covers both lifecycle states", () => {
    expect(TENANT_STATUS_META.active.label).toBe("Active");
    expect(TENANT_STATUS_META.suspended.label).toBe("Suspended");
  });
});
