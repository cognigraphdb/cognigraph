import { describe, expect, test } from "bun:test";
import { createRequestResult } from "./request-result.ts";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe("input-bound result publication", () => {
  test("a rerun clears data and timing immediately; failure cannot reveal the prior result", async () => {
    const store = createRequestResult<string>();
    await store.run(async () => "previous result");
    expect(store.getSnapshot()).toMatchObject({ status: "success", data: "previous result" });
    const failed = deferred<string>();
    const pending = store.run(() => failed.promise);
    expect(store.getSnapshot()).toEqual({ status: "pending" });
    failed.reject(new Error("server unavailable"));
    await pending;
    expect(store.getSnapshot()).toMatchObject({ status: "error", error: "server unavailable" });
    expect("data" in store.getSnapshot()).toBe(false);
    await store.run(async () => "recovered");
    expect(store.getSnapshot()).toMatchObject({ status: "success", data: "recovered" });
  });

  test("a result scope discarded on edit cannot publish data or success feedback later", async () => {
    const discarded = createRequestResult<string>();
    const current = createRequestResult<string>();
    const response = deferred<string>();
    const notices: string[] = [];
    const pending = discarded.run(
      () => response.promise,
      (data) => notices.push(data),
    );
    discarded.invalidate();
    await current.run(
      async () => "new inputs",
      (data) => notices.push(data),
    );
    response.resolve("old inputs");
    await pending;
    expect(discarded.getSnapshot()).toEqual({ status: "idle" });
    expect(current.getSnapshot()).toMatchObject({ status: "success", data: "new inputs" });
    expect(notices).toEqual(["new inputs"]);
  });

  test("reversed success completion keeps the newest execution's result and timing", async () => {
    const store = createRequestResult<string>();
    const slow = deferred<string>();
    const fast = deferred<string>();
    const notices: string[] = [];
    const older = store.run(
      () => slow.promise,
      (data) => notices.push(data),
    );
    const newer = store.run(
      () => fast.promise,
      (data) => notices.push(data),
    );
    fast.resolve("new result");
    await newer;
    const completed = store.getSnapshot();
    slow.resolve("old result");
    await older;
    expect(store.getSnapshot()).toBe(completed);
    expect(completed).toMatchObject({ status: "success", data: "new result" });
    expect(notices).toEqual(["new result"]);
  });

  test("an obsolete failure cannot clear the current pending state", async () => {
    const store = createRequestResult<string>();
    const slow = deferred<string>();
    const current = deferred<string>();
    const older = store.run(() => slow.promise);
    const newer = store.run(() => current.promise);
    slow.reject(new Error("obsolete failure"));
    await older;
    expect(store.getSnapshot()).toEqual({ status: "pending" });
    current.resolve("current result");
    await newer;
    expect(store.getSnapshot()).toMatchObject({ status: "success", data: "current result" });
  });

  test("late success cannot replace a newer failure", async () => {
    const store = createRequestResult<string>();
    const slow = deferred<string>();
    const older = store.run(() => slow.promise);
    await store.run(async () => {
      throw new Error("current failure");
    });
    const failure = store.getSnapshot();
    slow.resolve("obsolete success");
    await older;
    expect(store.getSnapshot()).toBe(failure);
    expect(failure).toMatchObject({ status: "error", error: "current failure" });
  });

  test("invalid local input clears an earlier success without invoking an API", async () => {
    const store = createRequestResult<string>();
    await store.run(async () => "previous result");
    await store.run(() => {
      throw new Error("invalid vector JSON");
    });
    expect(store.getSnapshot()).toMatchObject({ status: "error", error: "invalid vector JSON" });
    expect("data" in store.getSnapshot()).toBe(false);
  });

  test("returning to earlier input values does not resurrect a discarded result", async () => {
    const firstVisit = createRequestResult<string>();
    await firstVisit.run(async () => "earlier result");
    firstVisit.invalidate();
    const nextVisit = createRequestResult<string>();
    expect(firstVisit.getSnapshot()).toEqual({ status: "idle" });
    expect(nextVisit.getSnapshot()).toEqual({ status: "idle" });
  });
});
