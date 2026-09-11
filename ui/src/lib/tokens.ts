// API-token domain model + helpers. Shapes mirror the token routes in
// crates/cognigraph-server/src/routes/users.rs: listed tokens never include
// the hash; the plaintext appears exactly once, in the create/rotate grant.

export interface ApiTokenRecord {
  key: string;
  name: string;
  created_at: number;
  expires_at: number | null;
}

/// A freshly issued or rotated token. `token` is the plaintext `cg_…`
/// credential — the server stores only its hash, so this is the one and
/// only time it can be read.
export interface TokenGrant {
  key: string;
  token: string;
  expires_at: number | null;
}

/// TTL presets offered on creation. `undefined` = omit the field (server
/// default TTL); `0` = explicitly non-expiring, overriding the default.
export const TTL_PRESETS: Array<{ label: string; value: string; seconds: number | undefined }> = [
  { label: "Server default", value: "default", seconds: undefined },
  { label: "1 hour", value: "1h", seconds: 3_600 },
  { label: "1 day", value: "1d", seconds: 86_400 },
  { label: "30 days", value: "30d", seconds: 2_592_000 },
  { label: "Never expires", value: "never", seconds: 0 },
];

export function ttlSecondsFor(preset: string): number | undefined {
  return TTL_PRESETS.find((option) => option.value === preset)?.seconds;
}

/// Server timestamps are unix seconds; 0/undefined means "never".
export function formatUnixSeconds(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(seconds * 1000),
  );
}

export function describeExpiry(expiresAt: number | null | undefined, nowSecs: number): string {
  if (!expiresAt) return "Never expires";
  if (expiresAt <= nowSecs) return "Expired";
  return `Expires ${formatUnixSeconds(expiresAt)}`;
}
