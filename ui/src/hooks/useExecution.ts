import { useEffect, useMemo, useState } from "react";
import { createExecutionGuard, type IsCurrentExecution } from "../lib/execution-guard.ts";

// A connection change gets its own slot. The departing screen/connection cannot
// publish results, notifications or busy state when its request eventually ends.
export function useExecution(connection: object) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: each connection owns a separate execution slot.
  const guard = useMemo(() => createExecutionGuard(), [connection]);
  const [runningGuard, setRunningGuard] = useState<typeof guard>();
  useEffect(() => () => guard.invalidate(), [guard]);

  const execute = (operation: (isCurrent: IsCurrentExecution) => Promise<void>) =>
    guard.run(async (isCurrent) => {
      setRunningGuard(guard);
      try {
        await operation(isCurrent);
      } finally {
        if (isCurrent()) setRunningGuard(undefined);
      }
    });

  return { running: runningGuard === guard, execute };
}
