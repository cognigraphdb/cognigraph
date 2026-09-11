import { useCallback, useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { loadSpaceTypes } from "../lib/review-data.ts";

export function useSpaceTypes(api: CogniGraphApi) {
  const [revision, setRevision] = useState(0);
  const [result, setResult] = useState<{
    api: CogniGraphApi;
    revision: number;
    spaces: string[];
    error: string;
  }>();
  useEffect(() => {
    const controller = new AbortController();
    void loadSpaceTypes(api, controller.signal).then(
      (spaces) => {
        if (!controller.signal.aborted) setResult({ api, revision, spaces, error: "" });
      },
      (error) => {
        if (!controller.signal.aborted)
          setResult({
            api,
            revision,
            spaces: [],
            error: error instanceof Error ? error.message : "Unable to load spaces",
          });
      },
    );
    return () => controller.abort();
  }, [api, revision]);
  const current = result?.api === api && result.revision === revision ? result : undefined;
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  return {
    spaces: result?.api === api ? result.spaces : [],
    loading: !current,
    error: current?.error ?? "",
    refresh,
  };
}
