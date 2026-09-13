import { useCallback, useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import type { GraduationCandidate, GraduationResponse, NeuronDoc } from "../lib/neurons.ts";
import {
  loadNeuronPage,
  loadSelectedNeuron,
  type NeuronPage,
  type StatusFilter,
} from "../lib/review-data.ts";

const message = (error: unknown) =>
  error instanceof Error ? error.message : "Unable to load review data";

// Each request is bound to its inputs. A late response after a page, filter or
// selection change cannot replace the new queue or inspector.
export function useReviewData(
  api: CogniGraphApi,
  space: string | undefined,
  status: StatusFilter,
  page: number,
  selectedKey: string | undefined,
) {
  const [revision, setRevision] = useState(0);
  const queueKey = JSON.stringify([space, status, page, revision]);
  const flagKey = JSON.stringify([space, revision]);
  const selectionKey = JSON.stringify([space, selectedKey]);
  const [queue, setQueue] = useState<{
    api: CogniGraphApi;
    key: string;
    data?: NeuronPage;
    error: string;
  }>();
  const [flags, setFlags] = useState<{
    api: CogniGraphApi;
    key: string;
    data: GraduationCandidate[];
    error: string;
  }>();
  const [selection, setSelection] = useState<{
    api: CogniGraphApi;
    key: string;
    revision: number;
    data?: NeuronDoc;
    error: string;
  }>();

  useEffect(() => {
    if (!space) return;
    const controller = new AbortController();
    void loadNeuronPage(api, space, status, page, controller.signal).then(
      (data) => {
        if (!controller.signal.aborted) setQueue({ api, key: queueKey, data, error: "" });
      },
      (error) => {
        if (!controller.signal.aborted) setQueue({ api, key: queueKey, error: message(error) });
      },
    );
    return () => controller.abort();
  }, [api, space, status, page, queueKey]);

  useEffect(() => {
    if (!space) return;
    const controller = new AbortController();
    void api
      .request<GraduationResponse>(`/neurons/graduation?space_type=${encodeURIComponent(space)}`, {
        signal: controller.signal,
      })
      .then(
        (response) => {
          if (!controller.signal.aborted)
            setFlags({ api, key: flagKey, data: response.candidates, error: "" });
        },
        (error) => {
          if (!controller.signal.aborted)
            setFlags({ api, key: flagKey, data: [], error: message(error) });
        },
      );
    return () => controller.abort();
  }, [api, space, flagKey]);

  useEffect(() => {
    if (!space || !selectedKey) return;
    const controller = new AbortController();
    void loadSelectedNeuron(api, space, selectedKey, controller.signal).then(
      (data) => {
        if (!controller.signal.aborted)
          setSelection({ api, key: selectionKey, revision, data, error: "" });
      },
      (error) => {
        if (!controller.signal.aborted)
          setSelection({ api, key: selectionKey, revision, error: message(error) });
      },
    );
    return () => controller.abort();
  }, [api, space, selectedKey, selectionKey, revision]);

  const currentQueue = queue?.api === api && queue.key === queueKey ? queue : undefined;
  const currentFlags = flags?.api === api && flags.key === flagKey ? flags : undefined;
  const currentSelection =
    selection?.api === api && selection.key === selectionKey ? selection : undefined;
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  return {
    queue: currentQueue?.data,
    loading: Boolean(space) && !currentQueue,
    error: currentQueue?.error ?? "",
    flags: currentFlags?.data ?? [],
    flagsError: currentFlags?.error ?? "",
    selected: selectedKey ? currentSelection?.data : undefined,
    selectionLoading: Boolean(space && selectedKey) && currentSelection?.revision !== revision,
    selectionError: currentSelection?.error ?? "",
    refresh,
  };
}
