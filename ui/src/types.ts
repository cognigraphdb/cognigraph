export type EmbeddingState = "ready" | "missing" | "processing";
export type ConnectionStatus = "checking" | "online" | "offline";
export type JsonObject = Record<string, unknown>;
export type NoticeType = "success" | "error" | "info" | "warning";
export type Notify = (message: string, type?: NoticeType) => void;

export interface GraphDocument {
  /** The API document, untouched by display defaults and coercions. */
  raw: JsonObject;
  _id: string;
  _key: string;
  title: string;
  category: string;
  summary: string;
  owner: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  embedding: EmbeddingState;
  model?: string;
  dimensions?: number;
  content: Record<string, unknown>;
}

export type InspectorTab = "json" | "metadata";

export interface DocumentDraft {
  title: string;
  category: string;
  summary: string;
}

export interface ApiConfig {
  baseUrl: string;
  token: string;
}

export interface AuthSession {
  username: string;
  role: string;
  /// The tenant this session is scoped to — identity-scoped tenancy: the
  /// server routes every data call this token makes to it.
  tenant: string;
}

export interface HealthSnapshot {
  status: ConnectionStatus;
  service?: string;
  version?: string;
  database?: string;
  latencyMs?: number;
  error?: string;
}
