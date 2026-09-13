// Tenant domain model + helpers. Shapes mirror
// crates/cognigraph-server/src/routes/tenants.rs (TenantAdmin scope: only
// host-admin sessions pass the guard — D4, decision_multi_tenancy.md).
// Suspension preserves accounts and data; deletion removes credentials and
// quarantines stored data (decision_tenant_deletion.md).

export type TenantStatus = "active" | "suspended";

export interface TenantRecord {
  name: string;
  status: TenantStatus;
  created_at: number;
  /// max_active_jobs is enforced; other quota keys remain reserved schema.
  quotas?: Record<string, unknown> | null;
  /// Whether this tenant's data store is currently open in memory.
  store_open: boolean;
}

export interface TenantListResponse {
  tenants: TenantRecord[];
  count: number;
  open_stores: number;
}

export interface TenantDeletionResponse {
  deleted: boolean;
  quarantined: string[];
}

// A successful HTTP status alone cannot establish the destructive outcome.
export function parseTenantDeletion(value: unknown): TenantDeletionResponse {
  if (
    !value ||
    typeof value !== "object" ||
    !("deleted" in value) ||
    typeof value.deleted !== "boolean" ||
    !("quarantined" in value) ||
    !Array.isArray(value.quarantined) ||
    !value.quarantined.every((entry) => typeof entry === "string")
  ) {
    throw new Error("The server did not return a valid tenant deletion result.");
  }
  return { deleted: value.deleted, quarantined: value.quarantined };
}

export const TENANT_STATUS_META: Record<TenantStatus, { label: string; color: string }> = {
  active: { label: "Active", color: "green" },
  suspended: { label: "Suspended", color: "red" },
};

/// Only host-admin sessions can see or manage tenants (Scope::TenantAdmin).
export function canManageTenants(role: string | undefined): boolean {
  return role === "host-admin";
}
