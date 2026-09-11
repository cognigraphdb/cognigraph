import type { ApiConfig, HealthSnapshot, JsonObject } from "../types.ts";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

export class CogniGraphApi {
  /// Called when a request made WITH a bearer token gets a 401 — the session
  /// expired or was revoked. Lets the app shell drop back to the login
  /// screen instead of leaving the user clicking into errors.
  onUnauthorized?: () => void;

  constructor(private readonly config: ApiConfig) {}

  // Application endpoints live under `/api` (the UI owns `/`). Feature code
  // calls `get("/documents")` etc.; the `/api` prefix is applied here, once.
  // Operational routes (`/health`) stay at the root — see `fetchAt`.
  async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    return this.fetchAt<T>(`/api${path}`, init);
  }

  private async fetchAt<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);
    if (init.body) headers.set("Content-Type", "application/json");
    if (this.config.token) headers.set("Authorization", `Bearer ${this.config.token}`);
    const response = await fetch(`${this.config.baseUrl.replace(/\/$/, "")}${path}`, {
      ...init,
      headers,
    });
    const text = await response.text();
    const payload = text ? safeJson(text) : undefined;
    if (!response.ok) {
      if (response.status === 401 && this.config.token) this.onUnauthorized?.();
      const detail = isObject(payload)
        ? String(payload.error ?? payload.message ?? response.statusText)
        : response.statusText;
      throw new ApiError(detail || `Request failed with ${response.status}`, response.status);
    }
    return payload as T;
  }

  get<T>(path: string) {
    return this.request<T>(path);
  }

  post<T>(path: string, body?: unknown) {
    return this.request<T>(path, { method: "POST", body: body ? JSON.stringify(body) : undefined });
  }

  patch<T>(path: string, body: unknown) {
    return this.request<T>(path, { method: "PATCH", body: JSON.stringify(body) });
  }

  delete<T>(path: string) {
    return this.request<T>(path, { method: "DELETE" });
  }

  // Exchange credentials for a session token. POST /api/auth/login.
  async login(
    username: string,
    password: string,
  ): Promise<{ token: string; role: string; tenant: string }> {
    const res = await this.request<{
      token: string;
      role: string;
      tenant?: string;
      expires_in: number;
    }>("/auth/login", { method: "POST", body: JSON.stringify({ username, password }) });
    return { token: res.token, role: res.role, tenant: res.tenant ?? "default" };
  }

  // Does this server require a login the current token cannot satisfy? A
  // protected route's guard returns 401 before its handler runs when the token
  // is missing/invalid; with a valid token (or auth disabled) it returns
  // something else (e.g. 404 for an unknown collection). Any non-401 — including
  // a network error — is treated as "no login gate here", so we never trap the
  // user on the login screen when the server is simply unreachable.
  async authRequired(): Promise<boolean> {
    try {
      await this.request("/documents?collection=__auth_probe__&limit=1");
      return false;
    } catch (error) {
      return error instanceof ApiError && error.status === 401;
    }
  }

  // Prometheus exposition text from the root `/metrics` (not `/api`, not
  // JSON). Auth headers ride along harmlessly when present.
  async metricsText(): Promise<string> {
    const headers = new Headers();
    if (this.config.token) headers.set("Authorization", `Bearer ${this.config.token}`);
    const response = await fetch(`${this.config.baseUrl.replace(/\/$/, "")}/metrics`, { headers });
    if (!response.ok) {
      throw new ApiError(
        response.statusText || `Metrics failed with ${response.status}`,
        response.status,
      );
    }
    return response.text();
  }

  async health(): Promise<HealthSnapshot> {
    const started = performance.now();
    try {
      // Health/ops routes are at the root, not under `/api`.
      const [service, database] = await Promise.all([
        this.fetchAt<JsonObject>("/health"),
        this.fetchAt<JsonObject>("/health/database"),
      ]);
      return {
        status: service.status === "ok" ? "online" : "offline",
        service: String(service.service ?? "cognigraph"),
        version: String(service.version ?? "unknown"),
        database: String(database.database ?? database.status ?? "unknown"),
        latencyMs: Math.round(performance.now() - started),
      };
    } catch (error) {
      return {
        status: "offline",
        error: error instanceof Error ? error.message : "Server unavailable",
        latencyMs: Math.round(performance.now() - started),
      };
    }
  }
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
