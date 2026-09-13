import { useEffect, useMemo, useSyncExternalStore } from "react";
import { createRequestResult } from "../lib/request-result.ts";

export function useRequestResult<T>(connection: object, inputKey: string) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: these dependencies define result ownership, even for invalid/unsubmitted input.
  const store = useMemo(() => createRequestResult<T>(), [connection, inputKey]);
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  useEffect(() => () => store.invalidate(), [store]);
  return { state, run: store.run };
}
