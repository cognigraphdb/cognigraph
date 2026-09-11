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

  // Only a recognizable successful read establishes access. Network failures,
  // HTML fallbacks, permission errors and server failures leave auth unverified.
  async authRequired(): Promise<boolean> {
    try {
      const signal = AbortSignal.timeout(10_000);
      let payload: unknown;
      let catalog = "collections";
      try {
        payload = await this.request<unknown>("/collections", { signal });
      } catch (error) {
        // Host administrators manage tenants but cannot read tenant data. Verify
        // their protected control-plane catalog, never infer access from a 403.
        if (!(error instanceof ApiError) || error.status !== 403) throw error;
        catalog = "tenants";
        payload = await this.request<unknown>("/tenants", { signal });
      }
      if (!isObject(payload) || !Array.isArray(payload[catalog])) {
        throw new Error(`The server did not return a valid CogniGraph ${catalog} response.`);
      }
      return false;
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) return true;
      throw error;
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
        edition:
          service.edition === "community" || service.edition === "enterprise"
            ? service.edition
            : undefined,
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
