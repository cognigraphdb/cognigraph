import { expect, test } from "bun:test";
import { backoff, parseRetryAfter, retryPolicy, sleep } from "../../src/retry.js";

test("defaults are one attempt, 200 ms base and a 5 s ceiling", () => {
  expect(retryPolicy(undefined)).toEqual({ attempts: 1, baseMs: 200, maxMs: 5000 });
  expect(retryPolicy({ attempts: 4 })).toEqual({ attempts: 4, baseMs: 200, maxMs: 5000 });
});

test("backoff doubles, jitters below the step, honors Retry-After and caps everything", () => {
  const policy = retryPolicy({ attempts: 10, baseMs: 100, maxMs: 1000 });
  expect([1, 2, 3, 4, 5].map((n) => backoff(policy, n, undefined, () => 1))).toEqual([
    100, 200, 400, 800, 1000,
  ]);
  expect(backoff(policy, 3, undefined, () => 0)).toBe(0);
  expect(backoff(policy, 3, undefined, () => 0.5)).toBe(200);
  // The server's wait wins over a shorter jittered step, but never beyond maxMs.
  expect(backoff(policy, 1, 700, () => 0)).toBe(700);
  expect(backoff(policy, 1, 60_000, () => 0)).toBe(1000);
  // Huge attempt numbers do not overflow into Infinity or NaN.
  expect(backoff(policy, 5000, undefined, () => 1)).toBe(1000);
});

test("Retry-After accepts delta seconds and HTTP dates only", () => {
  const now = Date.parse("Wed, 23 Sep 2026 10:00:00 GMT");
  expect(parseRetryAfter(null, now)).toBeUndefined();
  expect(parseRetryAfter("0", now)).toBe(0);
  expect(parseRetryAfter(" 3 ", now)).toBe(3000);
  expect(parseRetryAfter("Wed, 23 Sep 2026 10:00:07 GMT", now)).toBe(7000);
  expect(parseRetryAfter("Wed, 23 Sep 2026 09:59:00 GMT", now)).toBe(0);
  for (const junk of ["", "-1", "1.5", "soon", "3s"]) {
    expect(parseRetryAfter(junk, now)).toBeUndefined();
  }
});

test("a sleep is cut short by its signal", async () => {
  const controller = new AbortController();
  const pending = sleep(10_000, controller.signal);
  controller.abort(new Error("stop"));
  await expect(pending).rejects.toThrow("stop");
  const aborted = AbortSignal.abort(new Error("already"));
  await expect(sleep(1, aborted)).rejects.toThrow("already");
  await sleep(1);
});
