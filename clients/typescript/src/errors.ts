/**
 * Typed errors. Every failure is a `CogniGraphError` carrying the HTTP status
 * (0 when no response arrived), the server's `error` message and optional
 * `code`, the parsed body, and whether a retry can help.
 */

export interface ErrorDetails {
  status: number;
  code?: string | undefined;
  body?: unknown;
  retryAfterMs?: number | undefined;
  cause?: unknown;
}

export class CogniGraphError extends Error {
  readonly status: number;
  readonly code: string | undefined;
  readonly body: unknown;
  /** Transient: 408, 429, 503, a client timeout or a network failure. */
  readonly retryable: boolean = false;
  /** From the server's `Retry-After` header, when present and valid. */
  readonly retryAfterMs: number | undefined;

  constructor(message: string, details: ErrorDetails) {
    super(message, details.cause === undefined ? undefined : { cause: details.cause });
    this.name = new.target.name;
    this.status = details.status;
    this.code = details.code;
    this.body = details.body;
    this.retryAfterMs = details.retryAfterMs;
  }
}

/** 400: CGQL syntax or validation failure, or a malformed request. */
export class BadRequestError extends CogniGraphError {}
/** 401: missing, expired or invalid credentials. */
export class AuthenticationError extends CogniGraphError {}
/** 403: the principal may not do this; `code` may be `enterprise_feature_required`. */
export class ForbiddenError extends CogniGraphError {}
/** 404: the document, collection or route does not exist. */
export class NotFoundError extends CogniGraphError {}
/** 409: a key or constraint conflict. */
export class ConflictError extends CogniGraphError {}
/** 409 with `code: "unique_violation"`: a declared unique constraint refused the write. */
export class UniqueViolationError extends ConflictError {}
/** 500: execution, storage or budget failure. Not retried. */
export class ServerError extends CogniGraphError {}

/** 408 from the server, or the client's own `timeoutMs` elapsing (status 0). */
export class TimeoutError extends CogniGraphError {
  override readonly retryable = true;
}
/** 429: rate or capacity limit; `retryAfterMs` says when to try again. */
export class RateLimitError extends CogniGraphError {
  override readonly retryable = true;
}
/** 503: the backend is unavailable. */
export class UnavailableError extends CogniGraphError {
  override readonly retryable = true;
}
/** No HTTP response: DNS, connection refused or reset. Status 0. */
export class NetworkError extends CogniGraphError {
  override readonly retryable = true;
}
/** A successful status with a body that is not the documented JSON. */
export class ProtocolError extends CogniGraphError {}

const BY_STATUS: Record<number, typeof CogniGraphError> = {
  400: BadRequestError,
  401: AuthenticationError,
  403: ForbiddenError,
  404: NotFoundError,
  408: TimeoutError,
  409: ConflictError,
  429: RateLimitError,
  500: ServerError,
  503: UnavailableError,
};

/** The typed error for an HTTP failure response. */
export function errorForStatus(status: number, body: unknown, text: string, retryAfterMs?: number) {
  const fields = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const message = typeof fields.error === "string" ? fields.error : text.trim() || `HTTP ${status}`;
  const code = typeof fields.code === "string" ? fields.code : undefined;
  const kind =
    status === 409 && code === "unique_violation"
      ? UniqueViolationError
      : (BY_STATUS[status] ?? (status >= 500 ? ServerError : CogniGraphError));
  return new kind(message, { status, code, body, retryAfterMs });
}
