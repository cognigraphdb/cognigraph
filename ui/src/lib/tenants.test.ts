import { describe, expect, test } from "bun:test";
import { canManageTenants, parseTenantDeletion, TENANT_STATUS_META } from "./tenants.ts";

describe("tenant deletion response", () => {
  test("preserves actual record deletion and quarantine results independently", () => {
    for (const deleted of [true, false]) {
      for (const quarantined of [[], ["qa.redb.deleted-123", "qa.tantivy.deleted-123"]]) {
        expect(parseTenantDeletion({ deleted, quarantined })).toEqual({ deleted, quarantined });
      }
    }
  });
  test("does not turn a malformed successful HTTP response into confirmed deletion", () => {
    for (const response of [
      null,
      "<html>fallback</html>",
      {},
      { deleted: "true", quarantined: [] },
      { deleted: true },
      { deleted: true, quarantined: [null] },
    ]) {
      expect(() => parseTenantDeletion(response)).toThrow("valid tenant deletion result");
    }
  });
});

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
