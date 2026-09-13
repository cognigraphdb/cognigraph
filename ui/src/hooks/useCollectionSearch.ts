import { useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { type CollectionSearchResult, loadCollectionSearch } from "../lib/collection-search.ts";
import type { ConnectionStatus, GraphDocument } from "../types.ts";

export function useCollectionSearch(
  api: CogniGraphApi,
  collection: string,
  query: string,
  connection: ConnectionStatus,
) {
  // Periodic health probes briefly report "checking". They do not invalidate
  // authenticated data or restart the current search.
  const connected = connection !== "offline";
  const [revision, setRevision] = useState(0);
  const owner = JSON.stringify([collection, query, revision]);
  const [settled, setSettled] = useState<{
    owner: string;
    api: CogniGraphApi;
    result: CollectionSearchResult;
  }>();
  useEffect(() => {
    setSettled(undefined);
    if (!query || !connected) return;
    let active = true;
    const timer = setTimeout(() => {
      void loadCollectionSearch(api, collection, query)
        .then((result) => {
          if (active) setSettled({ owner, api, result });
        })
        .catch((error: unknown) => {
          if (active)
            setSettled({
              owner,
              api,
              result: {
                documents: [],
                textLimitReached: false,
                errors: [
                  error instanceof Error ? error.message : "Search response could not be read",
                ],
              },
            });
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [api, collection, query, connected, owner]);
  const result =
    query && connected && settled?.owner === owner && settled.api === api
      ? settled.result
      : undefined;
  return {
    result,
    searching: Boolean(query && connected && !result),
    retry: () => setRevision((current) => current + 1),
    updateDocuments: (update: (documents: GraphDocument[]) => GraphDocument[]) =>
      setSettled((current) =>
        current?.owner === owner && current.api === api
          ? {
              ...current,
              result: { ...current.result, documents: update(current.result.documents) },
            }
          : current,
      ),
  };
}
