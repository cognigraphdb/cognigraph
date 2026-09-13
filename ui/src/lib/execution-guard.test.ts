import { describe, expect, test } from "bun:test";
import { createExecutionGuard, type IsCurrentExecution } from "./execution-guard.ts";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe("console execution ownership", () => {
  test("same-tick button, shortcut and form submissions perform one write", async () => {
    const guard = createExecutionGuard();
    const response = deferred();
    let writes = 0;
    const submit = () =>
      guard.run(async () => {
        writes += 1;
        await response.promise;
      });
    const first = submit();
    expect(await submit()).toBe(false);
    expect(await submit()).toBe(false);
    expect(writes).toBe(1);
    response.resolve();
    expect(await first).toBe(true);
    expect(await submit()).toBe(true);
    expect(writes).toBe(2);
  });

  test("the slot is claimed before caller code can re-enter submission", async () => {
    const guard = createExecutionGuard();
    let nested = Promise.resolve(true);
    let calls = 0;
    await guard.run(async () => {
      calls += 1;
      nested = guard.run(async () => {
        calls += 1;
      });
    });
    expect(await nested).toBe(false);
    expect(calls).toBe(1);
  });

  test("both a rejected request and a synchronous throw release the slot", async () => {
    const guard = createExecutionGuard();
    const response = deferred();
    const first = guard.run(() => response.promise);
    response.reject(new Error("offline"));
    await expect(first).rejects.toThrow("offline");
    await expect(
      guard.run(() => {
        throw new Error("invalid input");
      }),
    ).rejects.toThrow("invalid input");
    expect(await guard.run(async () => {})).toBe(true);
  });

  test("invalidation suppresses publication but retains the pending request slot", async () => {
    const guard = createExecutionGuard();
    const response = deferred();
    const publications: string[] = [];
    let ownership: IsCurrentExecution = () => false;
    const first = guard.run(async (isCurrent) => {
      ownership = isCurrent;
      await response.promise;
      if (isCurrent()) publications.push("obsolete result");
    });
    expect(ownership()).toBe(true);
    guard.invalidate();
    expect(ownership()).toBe(false);
    expect(
      await guard.run(async () => {
        publications.push("duplicate write");
      }),
    ).toBe(false);
    response.resolve();
    await first;
    await guard.run(async (isCurrent) => {
      expect(ownership()).toBe(false);
      if (isCurrent()) publications.push("current result");
    });
    expect(publications).toEqual(["current result"]);
  });

  test("a departed connection's completion cannot clear a newer execution's busy state", async () => {
    const previous = createExecutionGuard();
    const current = createExecutionGuard();
    const oldResponse = deferred();
    const newResponse = deferred();
    let busy = "previous";
    let result = "";
    const run = (
      gate: ReturnType<typeof createExecutionGuard>,
      response: ReturnType<typeof deferred>,
      label: string,
    ) =>
      gate.run(async (isCurrent) => {
        busy = label;
        await response.promise;
        if (isCurrent()) {
          result = label;
          busy = "";
        }
      });
    const oldRun = run(previous, oldResponse, "previous");
    previous.invalidate();
    const newRun = run(current, newResponse, "current");
    oldResponse.resolve();
    await oldRun;
    expect(busy).toBe("current");
    expect(result).toBe("");
    newResponse.resolve();
    await newRun;
    expect(busy).toBe("");
    expect(result).toBe("current");
  });
});
