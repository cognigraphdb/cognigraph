export type IsCurrentExecution = () => boolean;

// Claim the slot before invoking any caller code, without waiting for React to
// render a disabled button. Keep it until the actual request settles.
export function createExecutionGuard() {
  let inFlight: object | undefined;
  let generation = 0;

  return {
    // Suppress obsolete completions without pretending to cancel server writes.
    invalidate() {
      generation += 1;
    },
    async run(operation: (isCurrent: IsCurrentExecution) => Promise<void>): Promise<boolean> {
      if (inFlight) return false;
      const owner = {};
      const startedGeneration = generation;
      inFlight = owner;
      const isCurrent = () => inFlight === owner && generation === startedGeneration;
      try {
        await operation(isCurrent);
        return true;
      } finally {
        inFlight = undefined;
      }
    },
  };
}
