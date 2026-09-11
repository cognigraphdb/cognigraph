// Tenant domain model + helpers. Shapes mirror
// crates/cognigraph-server/src/routes/tenants.rs (TenantAdmin scope: only
// host-admin sessions pass the guard — D4, decision_multi_tenancy.md).
// M1 manages tenant RECORDS; suspension gates that tenant's users at the
// auth middleware.

export type TenantStatus = "active" | "suspended";

export interface TenantRecord {
  name: string;
  status: TenantStatus;
  created_at: number;
  /// Reserved quota schema (D5) — stored by the server, not yet enforced.
  quotas?: Record<string, unknown> | null;
  /// Whether this tenant's data store is currently open in memory.
  store_open: boolean;
}

export interface TenantListResponse {
  tenants: TenantRecord[];
  count: number;
  open_stores: number;
}

export const TENANT_STATUS_META: Record<TenantStatus, { label: string; color: string }> = {
  active: { label: "Active", color: "green" },
  suspended: { label: "Suspended", color: "red" },
};

/// Only host-admin sessions can see or manage tenants (Scope::TenantAdmin).
export function canManageTenants(role: string | undefined): boolean {
  return role === "host-admin";
}
