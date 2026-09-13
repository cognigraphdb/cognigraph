// Recent server error events from GET /api/admin/logs (admin scope).
// Shapes mirror crates/cognigraph-server/src/hardening.rs::ErrorEvent.

import type { JsonObject } from "../types.ts";

export interface LogEvent {
  /// Monotonic per-process sequence — a stable unique row key.
  seq: number;
  at: number;
  method: string;
  path: string;
  status: number;
  latencyMs: number;
  message: string;
  tenant?: string;
}

export function normalizeLogEvent(value: JsonObject): LogEvent {
  return {
    seq: typeof value.seq === "number" ? value.seq : 0,
    at: typeof value.at === "number" ? value.at : 0,
    method: String(value.method ?? ""),
    path: String(value.path ?? ""),
    status: typeof value.status === "number" ? value.status : Number(value.status ?? 0),
    latencyMs: typeof value.latency_ms === "number" ? value.latency_ms : 0,
    message: String(value.message ?? ""),
    tenant: typeof value.tenant === "string" ? value.tenant : undefined,
  };
}

/// The severity band of a status code, for styling.
export function statusClass(status: number): "server" | "client" | "other" {
  if (status >= 500) return "server";
  if (status >= 400) return "client";
  return "other";
}

/// Clock time (HH:MM:SS) from a unix-seconds timestamp.
export function formatEventTime(at: number): string {
  if (!at) return "—";
  return new Date(at * 1000).toLocaleTimeString();
}
