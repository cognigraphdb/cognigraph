import { Button, Input, Modal } from "antd";
import { useRef, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { type ImportReview, reviewImport, submitImport } from "../lib/construct-import.ts";
import type { JsonObject } from "../types.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

interface Props {
  api: CogniGraphApi;
  space?: string;
  disabled: boolean;
  onBusy: (busy: boolean) => void;
  onComplete: (response: JsonObject) => void;
}

export function ConstructIngest({ api, space, disabled, onBusy, onComplete }: Props) {
  const inFlight = useRef(false);
  const reviewButton = useRef<HTMLButtonElement>(null);
  const [input, setInput] = useState("");
  const [review, setReview] = useState<ImportReview>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const existing = review?.stored.filter(Boolean).length ?? 0;

  const prepare = async () => {
    if (!space || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    onBusy(true);
    setError("");
    try {
      setReview(await reviewImport(api, space, input));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not review import.");
      onBusy(false);
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  };

  const closeReview = () => {
    setReview(undefined);
    onBusy(false);
    // Conditional Modal unmount skips Ant's close-animation focus restoration.
    // Wait until the opener is enabled again after the parent clears its busy state.
    requestAnimationFrame(() => reviewButton.current?.focus());
  };

  const cancel = () => {
    if (!inFlight.current) closeReview();
  };

  const submit = async () => {
    if (!review || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError("");
    try {
      onComplete(await submitImport(api, review));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Import failed. Review and retry.");
    } finally {
      inFlight.current = false;
      setBusy(false);
      closeReview();
    }
  };

  return (
    <>
      <Input.TextArea
        aria-label="Ingest corpus"
        autoSize={{ minRows: 2, maxRows: 5 }}
        disabled={disabled}
        onChange={(event) => setInput(event.target.value)}
        placeholder={'Chunks: plain text lines, or JSONL {"id", "title", "text"}'}
        value={input}
      />
      <p>
        Generated IDs follow the text and title, so retries reuse them. Supply JSONL ids to
        deliberately replace a source.
      </p>
      {error ? <ErrorAlert title={error} /> : null}
      <div className="actions-row-base">
        <Button
          ref={reviewButton}
          disabled={disabled || !space || !input.trim()}
          loading={busy && !review}
          onClick={() => void prepare()}
        >
          Review import
        </Button>
      </div>
      {review ? (
        <Modal
          open
          title="Review chunk import"
          width={760}
          onCancel={cancel}
          onOk={() => void submit()}
          confirmLoading={busy}
          cancelButtonProps={{ disabled: busy }}
          closable={!busy}
          keyboard={!busy}
          mask={{ closable: !busy }}
          okButtonProps={{ danger: existing > 0 }}
          okText={existing ? "Replace evidence and ground" : "Ground new chunks"}
        >
          <p>
            Space: <strong>{review.space}</strong>. {review.chunks.length - existing} new
            {review.chunks.length - existing === 1 ? " chunk" : " chunks"}; {existing} existing
            {existing === 1 ? " chunk" : " chunks"}.
          </p>
          {existing > 0 ? (
            <p role="alert">
              Continuing replaces the source text and rebuilds derived facts and mentions for the{" "}
              {existing} existing {existing === 1 ? "chunk" : "chunks"}, even when the text is
              unchanged. Earlier evidence may disappear. Cancel to keep it.
            </p>
          ) : (
            <p>This import adds new source identities. Review the text before grounding.</p>
          )}
          {/* biome-ignore lint/a11y/noNoninteractiveTabindex: This bounded region must support keyboard scrolling through all source text. */}
          <section className="import-preview" tabIndex={0} aria-label="Chunk import preview">
            {review.chunks.map((chunk, index) => (
              <section className="import-preview-chunk" key={chunk.id}>
                <strong>
                  {review.stored[index] ? "Replace" : "New"}: {chunk.id}
                </strong>
                {review.stored[index] ? (
                  <>
                    <span>Current source</span>
                    <pre>
                      {JSON.stringify(
                        { title: review.stored[index]?.title, text: review.stored[index]?.text },
                        null,
                        2,
                      )}
                    </pre>
                  </>
                ) : null}
                <span>Incoming source</span>
                <pre>
                  {JSON.stringify({ title: chunk.title ?? null, text: chunk.text }, null, 2)}
                </pre>
              </section>
            ))}
          </section>
        </Modal>
      ) : null}
    </>
  );
}
