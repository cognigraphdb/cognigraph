import { CircleNotch } from "@phosphor-icons/react";
import type { RequestResult } from "../lib/request-result.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

export function RequestFeedback({
  state,
  idle = "Run the request to see results. Results clear when inputs change.",
  waiting = false,
}: {
  state: RequestResult<unknown>;
  idle?: string;
  waiting?: boolean;
}) {
  if (state.status === "success") return null;
  return (
    <div className="empty-state-base result-empty graph-empty-state">
      {state.status === "error" ? (
        <ErrorAlert title={state.error} />
      ) : (
        <div role="status">
          {state.status === "pending" ? (
            <>
              <CircleNotch aria-hidden="true" className="cg-spin" /> Running request…
            </>
          ) : waiting ? (
            "Inputs changed. The previous execution is still finishing."
          ) : (
            idle
          )}
        </div>
      )}
    </div>
  );
}
