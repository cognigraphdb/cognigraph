/** Request and response shapes of the CogniGraph HTTP API used by the client. */

export type BindVars = Record<string, unknown>;

/** A stored document: user fields plus the server-owned identity fields. */
export type StoredDocument<T extends object = Record<string, unknown>> = T & {
  _key: string;
  _id: string;
};

export type BatchOp =
  | { op: "insert"; collection: string; doc: Record<string, unknown> }
  | { op: "update"; collection: string; key: string; merge: Record<string, unknown> }
  | { op: "replace"; collection: string; key: string; doc: Record<string, unknown> }
  | { op: "delete"; collection: string; key: string };

export interface CreatedDocument {
  _id: string;
  _key: string;
  collection: string;
}

export interface DeletedDocument {
  deleted: boolean;
  _id: string;
  side_views_deleted?: number;
}

export interface ListOptions {
  limit?: number;
  offset?: number;
}

/** `POST /api/collections/{name}/indexes`; `unique` defaults to true on the server. */
export interface IndexDefinition {
  fields: string[];
  unique?: boolean;
  sparse?: boolean;
  name?: string;
  index_type?: "persistent" | "hash";
}

export interface IndexDescription {
  index_type: string;
  fields: string[];
  unique: boolean;
  sparse: boolean;
  name: string | null;
}

export interface LoginResult {
  token: string;
  /** Seconds until the JWT expires. */
  expiresIn: number;
}

export interface DatabaseHealth {
  /** True for HTTP 200; a 503 is reported, not thrown. */
  ok: boolean;
  status: number;
  body: unknown;
}

export interface CallOptions {
  signal?: AbortSignal;
  /** Overrides the client's `timeoutMs` for this call. */
  timeoutMs?: number;
  /** Mark a raw `request()` as safe to retry under the client's retry policy. */
  idempotent?: boolean;
}
