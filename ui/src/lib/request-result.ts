export type RequestResult<T> =
  | { status: "idle" | "pending" }
  | { status: "success"; data: T; elapsed: number }
  | { status: "error"; error: string; elapsed: number };

// A result belongs to one input scope and one execution within that scope.
// Starting again clears the previous result; only the latest owner can publish.
export function createRequestResult<T>() {
  let state: RequestResult<T> = { status: "idle" };
  let revision = 0;
  const listeners = new Set<() => void>();
  const publish = (next: RequestResult<T>) => {
    state = next;
    for (const listener of listeners) listener();
  };
  return {
    getSnapshot: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    invalidate() {
      revision += 1;
      publish({ status: "idle" });
    },
    async run(work: () => Promise<T>, onSuccess?: (data: T) => void) {
      const owner = ++revision;
      const started = performance.now();
      publish({ status: "pending" });
      try {
        const data = await work();
        if (owner !== revision) return;
        publish({ status: "success", data, elapsed: Math.round(performance.now() - started) });
        if (owner === revision) onSuccess?.(data);
      } catch (error) {
        if (owner !== revision) return;
        publish({
          status: "error",
          error: error instanceof Error ? error.message : "Request failed",
          elapsed: Math.round(performance.now() - started),
        });
      }
    },
  };
}
