/** Opt-in retry policy for idempotent calls. */

export interface RetryOptions {
  /** Total attempts including the first; at least 1. */
  attempts: number;
  /** First backoff step in milliseconds (default 200). */
  baseMs?: number;
  /** Ceiling for any single wait, including a server `Retry-After` (default 5000). */
  maxMs?: number;
}

export interface RetryPolicy {
  attempts: number;
  baseMs: number;
  maxMs: number;
}

export function retryPolicy(options: RetryOptions | undefined): RetryPolicy {
  const policy = { attempts: 1, baseMs: 200, maxMs: 5000, ...options };
  if (!Number.isInteger(policy.attempts) || policy.attempts < 1) {
    throw new RangeError("retry.attempts must be an integer of at least 1");
  }
  for (const [name, value] of [
    ["baseMs", policy.baseMs],
    ["maxMs", policy.maxMs],
  ] as const) {
    if (!Number.isFinite(value) || value < 0) {
      throw new RangeError(`retry.${name} must be a non-negative finite number`);
    }
  }
  return policy;
}

/** RFC 9110 IMF-fixdate, e.g. `Wed, 23 Sep 2026 10:00:07 GMT`. */
const HTTP_DATE = /^[A-Z][a-z]{2}, \d{2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT$/;

/**
 * Milliseconds from a `Retry-After` header: delta seconds or an HTTP date.
 * Anything else is ignored; `Date.parse` alone would accept `-1` or `1.5`.
 */
export function parseRetryAfter(value: string | null, now = Date.now()): number | undefined {
  if (value === null) return undefined;
  const trimmed = value.trim();
  if (/^\d+$/.test(trimmed)) return Number(trimmed) * 1000;
  if (!HTTP_DATE.test(trimmed)) return undefined;
  const date = Date.parse(trimmed);
  return Number.isNaN(date) ? undefined : Math.max(0, date - now);
}

/**
 * Wait before attempt `attempt + 1` (attempt counts from 1): full-jitter
 * exponential backoff, never shorter than the server's `Retry-After`, never
 * longer than `maxMs`.
 */
export function backoff(
  policy: RetryPolicy,
  attempt: number,
  retryAfterMs: number | undefined,
  random: () => number = Math.random,
): number {
  const exponential = Math.min(policy.maxMs, policy.baseMs * 2 ** (attempt - 1));
  const jittered = random() * exponential;
  return Math.min(policy.maxMs, Math.max(jittered, retryAfterMs ?? 0));
}

export function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(signal.reason);
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", abort);
      resolve();
    }, ms);
    const abort = () => {
      clearTimeout(timer);
      reject(signal?.reason);
    };
    signal?.addEventListener("abort", abort, { once: true });
  });
}
