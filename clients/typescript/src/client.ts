import {
  CogniGraphError,
  errorForStatus,
  NetworkError,
  ProtocolError,
  TimeoutError,
} from "./errors.js";
import { backoff, parseRetryAfter, type RetryOptions, retryPolicy, sleep } from "./retry.js";
import type {
  BatchOp,
  BindVars,
  CallOptions,
  CreatedDocument,
  DatabaseHealth,
  DeletedDocument,
  IndexDefinition,
  IndexDescription,
  ListOptions,
  LoginResult,
  StoredDocument,
} from "./types.js";

export interface ClientOptions {
  /** Server origin, optionally with a path prefix, e.g. `http://127.0.0.1:3000`. */
  baseUrl: string;
  /** Bearer token: a login JWT or an API token. */
  token?: string;
  /** Per-attempt timeout in milliseconds (default 30000). */
  timeoutMs?: number;
  /** Opt-in retries for idempotent calls; off when omitted. */
  retry?: RetryOptions;
  /** Extra headers sent with every request. */
  headers?: Record<string, string>;
  /** `fetch` implementation (defaults to the global one). */
  fetch?: typeof globalThis.fetch;
}

interface Send extends CallOptions {
  /** A status that is returned instead of thrown. */
  accept?: number;
}

interface Received {
  status: number;
  body: unknown;
}

type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

const segment = encodeURIComponent;

/**
 * A CogniGraph HTTP client for Bun and Node (20.3+). Stateless: a token is
 * fixed per instance; `withToken` derives a new one. Reads can be retried
 * (`retry` option); mutations, batches and other writes never are.
 */
export class CogniGraph {
  readonly options: Readonly<ClientOptions>;
  private readonly base: string;
  private readonly policy;
  private readonly timeoutMs: number;
  private readonly doFetch: typeof globalThis.fetch;

  constructor(options: ClientOptions) {
    const url = new URL(options.baseUrl);
    this.base = url.href.replace(/\/+$/, "");
    this.timeoutMs = options.timeoutMs ?? 30_000;
    if (!Number.isFinite(this.timeoutMs) || this.timeoutMs <= 0) {
      throw new RangeError("timeoutMs must be a positive finite number");
    }
    this.policy = retryPolicy(options.retry);
    this.doFetch = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.options = Object.freeze({ ...options });
  }

  /** The same client authenticated with `token`. */
  withToken(token: string): CogniGraph {
    return new CogniGraph({ ...this.options, token });
  }

  /** Exchange credentials for a short-lived JWT. The client itself is unchanged. */
  async login(username: string, password: string, options?: CallOptions): Promise<LoginResult> {
    const body = (await this.call("POST", "/api/auth/login", { username, password }, options)) as {
      token: string;
      expires_in: number;
    };
    return { token: body.token, expiresIn: body.expires_in };
  }

  /** Read-only CGQL (`POST /api/search/query`); retried under the retry policy. */
  async query<T = unknown>(query: string, bindVars: BindVars = {}, options?: CallOptions) {
    const body = { query, bind_vars: bindVars };
    return results<T>(
      await this.call("POST", "/api/search/query", body, { ...options, idempotent: true }),
    );
  }

  /** CGQL mutations (`POST /api/query`); never retried automatically. */
  async mutate<T = unknown>(query: string, bindVars: BindVars = {}, options?: CallOptions) {
    const body = { query, bind_vars: bindVars };
    return results<T>(await this.call("POST", "/api/query", body, noRetry(options)));
  }

  /** Atomic multi-operation write: every op applies or none does. */
  async batch<T = unknown>(ops: BatchOp[], options?: CallOptions) {
    return results<T>(await this.call("POST", "/api/batch", { ops }, noRetry(options)));
  }

  readonly documents = {
    /** The document, or `null` when it does not exist. */
    get: async <T extends object = Record<string, unknown>>(
      collection: string,
      key: string,
      options?: CallOptions,
    ): Promise<StoredDocument<T> | null> => {
      const path = `/api/documents/${segment(collection)}/${segment(key)}`;
      const reply = await this.send("GET", path, undefined, {
        ...options,
        idempotent: true,
        accept: 404,
      });
      return reply.status === 404 ? null : (reply.body as StoredDocument<T>);
    },
    list: async <T extends object = Record<string, unknown>>(
      collection: string,
      page: ListOptions = {},
      options?: CallOptions,
    ): Promise<StoredDocument<T>[]> => {
      const query = new URLSearchParams({ collection });
      if (page.limit !== undefined) query.set("limit", String(page.limit));
      if (page.offset !== undefined) query.set("offset", String(page.offset));
      const body = await this.call("GET", `/api/documents?${query}`, undefined, {
        ...options,
        idempotent: true,
      });
      return results<StoredDocument<T>>(body);
    },
    /** `doc` must not have a `collection` field; the API reserves that name. */
    create: async (collection: string, doc: Record<string, unknown>, options?: CallOptions) => {
      if ("collection" in doc) {
        throw new TypeError(
          "documents.create: `collection` is reserved by the API; rename the field",
        );
      }
      return (await this.call(
        "POST",
        "/api/documents",
        { collection, ...doc },
        noRetry(options),
      )) as CreatedDocument;
    },
    /** Partial update (merge). */
    update: async <T extends object = Record<string, unknown>>(
      collection: string,
      key: string,
      patch: Record<string, unknown>,
      options?: CallOptions,
    ) =>
      (await this.call(
        "PATCH",
        docPath(collection, key),
        patch,
        noRetry(options),
      )) as StoredDocument<T>,
    /** Full replacement; fields not in `doc` are removed. */
    replace: async <T extends object = Record<string, unknown>>(
      collection: string,
      key: string,
      doc: Record<string, unknown>,
      options?: CallOptions,
    ) =>
      (await this.call(
        "PUT",
        docPath(collection, key),
        doc,
        noRetry(options),
      )) as StoredDocument<T>,
    delete: async (collection: string, key: string, options?: CallOptions) =>
      (await this.call(
        "DELETE",
        docPath(collection, key),
        undefined,
        noRetry(options),
      )) as DeletedDocument,
  };

  readonly indexes = {
    list: async (collection: string, options?: CallOptions): Promise<IndexDescription[]> => {
      const body = (await this.call("GET", indexPath(collection), undefined, {
        ...options,
        idempotent: true,
      })) as { indexes: IndexDescription[] };
      return body.indexes;
    },
    /** Declare a unique constraint (unless `unique: false`); idempotent on the server. */
    ensure: async (collection: string, definition: IndexDefinition, options?: CallOptions) =>
      (await this.call("POST", indexPath(collection), definition, noRetry(options))) as {
        collection: string;
        index: IndexDescription;
      },
    drop: async (collection: string, name: string, options?: CallOptions) =>
      (await this.call(
        "DELETE",
        `${indexPath(collection)}/${segment(name)}`,
        undefined,
        noRetry(options),
      )) as { collection: string; index: string; dropped: boolean },
  };

  /** `GET /health`: liveness, version and edition. */
  async health(options?: CallOptions): Promise<Record<string, unknown>> {
    return (await this.call("GET", "/health", undefined, {
      ...options,
      idempotent: true,
    })) as Record<string, unknown>;
  }

  /** `GET /health/database`: a 503 is reported as `ok: false`, not thrown. */
  async databaseHealth(options?: CallOptions): Promise<DatabaseHealth> {
    const reply = await this.send("GET", "/health/database", undefined, {
      ...options,
      idempotent: true,
      accept: 503,
    });
    return { ok: reply.status === 200, status: reply.status, body: reply.body };
  }

  /** Escape hatch for routes without a helper; `idempotent: true` allows retries. */
  async request<T = unknown>(method: Method, path: string, body?: unknown, options?: CallOptions) {
    return (await this.call(method, path, body, { idempotent: false, ...options })) as T;
  }

  private async call(method: Method, path: string, body: unknown, options?: Send) {
    return (await this.send(method, path, body, options)).body;
  }

  private async send(
    method: Method,
    path: string,
    body: unknown,
    options: Send = {},
  ): Promise<Received> {
    const attempts = options.idempotent ? this.policy.attempts : 1;
    for (let attempt = 1; ; attempt++) {
      try {
        return await this.once(method, path, body, options);
      } catch (error) {
        const retry = error instanceof CogniGraphError && error.retryable && attempt < attempts;
        if (!retry) throw error;
        await sleep(backoff(this.policy, attempt, error.retryAfterMs), options.signal);
      }
    }
  }

  private async once(
    method: Method,
    path: string,
    body: unknown,
    options: Send,
  ): Promise<Received> {
    const timeoutMs = options.timeoutMs ?? this.timeoutMs;
    const timeout = AbortSignal.timeout(timeoutMs);
    const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
    const headers: Record<string, string> = { accept: "application/json", ...this.options.headers };
    if (body !== undefined) headers["content-type"] = "application/json";
    if (this.options.token) headers.authorization = `Bearer ${this.options.token}`;
    let response: Response;
    try {
      response = await this.doFetch(`${this.base}/${path.replace(/^\/+/, "")}`, {
        method,
        headers,
        signal,
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
    } catch (cause) {
      if (options.signal?.aborted) throw options.signal.reason;
      if (timeout.aborted) {
        throw new TimeoutError(`no response within ${timeoutMs} ms`, { status: 0, cause });
      }
      throw new NetworkError(cause instanceof Error ? cause.message : String(cause), {
        status: 0,
        cause,
      });
    }
    const text = await response.text();
    let parsed: unknown;
    let valid = true;
    try {
      parsed = text === "" ? null : JSON.parse(text);
    } catch {
      valid = false;
    }
    if (response.ok || response.status === options.accept) {
      if (!valid) {
        throw new ProtocolError(`expected JSON from ${method} ${path}`, {
          status: response.status,
          body: text,
        });
      }
      return { status: response.status, body: parsed };
    }
    const retryAfter = parseRetryAfter(response.headers.get("retry-after"));
    throw errorForStatus(
      response.status,
      valid ? parsed : undefined,
      valid ? "" : text,
      retryAfter,
    );
  }
}

function noRetry(options?: CallOptions): Send {
  return { ...options, idempotent: false };
}

function results<T>(body: unknown): T[] {
  const rows = (body as { results?: unknown } | null)?.results;
  if (!Array.isArray(rows)) {
    throw new ProtocolError("response has no `results` array", { status: 200, body });
  }
  return rows as T[];
}

function docPath(collection: string, key: string) {
  return `/api/documents/${segment(collection)}/${segment(key)}`;
}

function indexPath(collection: string) {
  return `/api/collections/${segment(collection)}/indexes`;
}
