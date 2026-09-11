import {
  ChartBar,
  CircleNotch,
  Compass,
  FlowArrow,
  Gavel,
  Lightbulb,
  PencilRuler,
} from "@phosphor-icons/react";
import { Button, Checkbox, Input, InputNumber, Select } from "antd";
import type { ReactNode } from "react";
import { useCallback, useEffect, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { ConstructIngest } from "../components/ConstructIngest.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { type ConstructAction, parseChunks, parseGaps, summarizeRun } from "../lib/construct.ts";
import type { JsonObject, Notify } from "../types.ts";

interface ConstructScreenProps {
  api: CogniGraphApi;
  notify: Notify;
}

/// The governed construction pipeline, one stage per row: draft an
/// ontology (LLM, inert until accepted), accept it, ground chunks into
/// fact edges, measure recall/restraint, propose repairs, run the judge,
/// and ask the gate advisor. Stages that need the server's completion
/// provider fail with its actionable message when none is configured.
export function ConstructScreen({ api, notify }: ConstructScreenProps) {
  const { constructWrite } = useAccess();
  const [spaces, setSpaces] = useState<string[]>([]);
  const [space, setSpace] = useState<string>();
  const [result, setResult] = useState<unknown>();
  const [running, setRunning] = useState<string>();

  const [draftId, setDraftId] = useState("");
  const [draftChunks, setDraftChunks] = useState("");
  const [gaps, setGaps] = useState("");
  const [reviewLimit, setReviewLimit] = useState<number>();
  const [rejudge, setRejudge] = useState(false);

  const loadSpaces = useCallback(() => {
    api
      .get<{ results: JsonObject[] }>("/documents?collection=space_types&limit=100")
      .then(({ results }) => {
        const names = results.map((doc) => String(doc._key)).sort();
        setSpaces(names);
        setSpace((current) => current ?? names[0]);
      })
      .catch(() => setSpaces([]));
  }, [api]);
  useEffect(loadSpaces, [loadSpaces]);

  /// Run one stage; the shared result panel always shows the raw response.
  const run = async (
    stage: string,
    action: ConstructAction,
    request: () => { path: string; body: JsonObject } | Promise<{ path: string; body: JsonObject }>,
  ) => {
    if (!["evaluate", "advise"].includes(action) && !constructWrite) return;
    setRunning(stage);
    try {
      const call = await request();
      const response = await api.post<JsonObject>(call.path, call.body);
      setResult(response);
      notify(summarizeRun(action, response));
      if (action === "accept") loadSpaces();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Run failed";
      setResult({ error: message });
      notify(message, "error");
    } finally {
      setRunning(undefined);
    }
  };

  const needsSpace = () => {
    if (!space) throw new Error("Pick a space first — or draft one below.");
    return space;
  };

  return (
    <main className="page-workspace">
      <PageHeader
        description={
          constructWrite
            ? "Draft an ontology, ground chunks into evidence-bound facts, measure recall and restraint, and run the repair loop."
            : "Read-only access: evaluate an existing space or run its gate advisor. Your role cannot draft, ingest, propose or run judge review."
        }
        eyebrow="Knowledge / construction"
        title="Construct pipeline"
      />
      <div className="construct-toolbar">
        <span>Space</span>
        <Select
          aria-label="Space type"
          disabled={Boolean(running) || spaces.length === 0}
          onChange={setSpace}
          options={spaces.map((name) => ({ label: name, value: name }))}
          placeholder={spaces.length === 0 ? "None in this tenant — draft one" : "Select"}
          value={space}
        />
      </div>
      <div className="operations-grid construct-grid">
        <section className="console-card action-list">
          <Stage
            icon={PencilRuler}
            title="Draft an ontology"
            detail="An LLM proposes a NEW space from sample chunks — entities and naive rules, symbolically checked, inert until accepted."
          >
            <Input
              aria-label="New space id"
              onChange={(event) => setDraftId(event.target.value)}
              placeholder="new space id, e.g. demo_meds"
              value={draftId}
            />
            <Input.TextArea
              aria-label="Draft corpus"
              autoSize={{ minRows: 2, maxRows: 5 }}
              onChange={(event) => setDraftChunks(event.target.value)}
              placeholder={'Chunks: plain text lines, or JSONL {"id", "title", "text"}'}
              value={draftChunks}
            />
            <div className="actions-row-base">
              <RunButton
                busy={running === "draft"}
                disabled={
                  !constructWrite || Boolean(running) || !draftId.trim() || !draftChunks.trim()
                }
                label="Draft"
                onClick={() =>
                  void run("draft", "draft", async () => ({
                    path: "/construct/draft",
                    body: { space_type: draftId.trim(), chunks: await parseChunks(draftChunks) },
                  }))
                }
              />
              <RunButton
                busy={running === "accept"}
                disabled={!constructWrite || Boolean(running) || !draftId.trim()}
                label="Accept draft"
                onClick={() =>
                  void run("accept", "accept", () => ({
                    path: `/construct/draft/${encodeURIComponent(draftId.trim())}/accept`,
                    body: {},
                  }))
                }
              />
            </div>
          </Stage>
          <Stage
            icon={FlowArrow}
            title="Ingest"
            detail="Ground chunks into evidence-bound fact edges under the selected space's rules — deterministic, accepted neurons applied."
          >
            <ConstructIngest
              api={api}
              space={space}
              disabled={!constructWrite || Boolean(running)}
              onBusy={(busy) => setRunning(busy ? "ingest" : undefined)}
              onComplete={(response) => {
                setResult(response);
                notify(summarizeRun("ingest", response));
              }}
            />
          </Stage>
          <Stage
            icon={ChartBar}
            title="Evaluate"
            detail="Recall (expected facts built) and restraint (forbidden facts NOT built) against the space's stored eval spec."
          >
            <div className="actions-row-base">
              <RunButton
                busy={running === "evaluate"}
                disabled={Boolean(running) || !space}
                label="Measure"
                onClick={() =>
                  void run("evaluate", "evaluate", () => ({
                    path: "/construct/evaluate",
                    body: { space_type: needsSpace() },
                  }))
                }
              />
            </div>
          </Stage>
          <Stage
            icon={Lightbulb}
            title="Propose repairs"
            detail="An LLM proposes neurons for the gaps — stored as proposed (inert) with authorship; accept them on the Review page."
          >
            <Input.TextArea
              aria-label="Gaps"
              autoSize={{ minRows: 1, maxRows: 4 }}
              onChange={(event) => setGaps(event.target.value)}
              placeholder="Optional gaps, one `A --REL--> B` per line; empty = measure live via the eval spec"
              value={gaps}
            />
            <div className="actions-row-base">
              <RunButton
                busy={running === "propose"}
                disabled={!constructWrite || Boolean(running) || !space}
                label="Propose"
                onClick={() =>
                  void run("propose", "propose", () => ({
                    path: "/construct/propose",
                    body: {
                      space_type: needsSpace(),
                      ...(gaps.trim() ? { gaps: parseGaps(gaps) } : {}),
                    },
                  }))
                }
              />
            </div>
          </Stage>
          <Stage
            icon={Gavel}
            title="Judge review run"
            detail="The measured judge works the proposed queue: the safe lane may auto-accept under the space's policy; everything else queues for a human."
          >
            <div className="actions-row-base">
              <InputNumber
                aria-label="Review limit"
                controls={false}
                min={1}
                onChange={(value) => setReviewLimit(value ?? undefined)}
                placeholder="limit"
                value={reviewLimit}
              />
              <Checkbox checked={rejudge} onChange={(event) => setRejudge(event.target.checked)}>
                Re-judge
              </Checkbox>
              <RunButton
                busy={running === "review"}
                disabled={!constructWrite || Boolean(running) || !space}
                label="Run judge"
                onClick={() =>
                  void run("review", "review", () => ({
                    path: "/construct/review",
                    body: {
                      space_type: needsSpace(),
                      ...(reviewLimit ? { limit: reviewLimit } : {}),
                      ...(rejudge ? { rejudge: true } : {}),
                    },
                  }))
                }
              />
            </div>
          </Stage>
          <Stage
            icon={Compass}
            title="Gate advisor"
            detail="Deterministic, read-only: where a require_in_sentence gate is safe, and which endpoints look like leakage (flagged, never auto-applied)."
          >
            <div className="actions-row-base">
              <RunButton
                busy={running === "advise"}
                disabled={Boolean(running) || !space}
                label="Advise"
                onClick={() =>
                  void run("advise", "advise", () => ({
                    path: "/construct/advise",
                    body: { space_type: needsSpace() },
                  }))
                }
              />
            </div>
          </Stage>
        </section>
        <section className="console-card result-panel operation-result">
          <div className="card-heading compact-heading">
            <h2>Run result</h2>
          </div>
          <div className="result-body">
            <JsonResult empty="Run a pipeline stage to inspect its response." value={result} />
          </div>
        </section>
      </div>
    </main>
  );
}

function Stage({
  icon: Icon,
  title,
  detail,
  children,
}: {
  icon: typeof FlowArrow;
  title: string;
  detail: string;
  children: ReactNode;
}) {
  return (
    <article className="operation-row construct-stage">
      <div className="operation-icon">
        <Icon aria-hidden="true" size={21} />
      </div>
      <div className="construct-stage-body">
        <strong>{title}</strong>
        <p>{detail}</p>
        {children}
      </div>
    </article>
  );
}

function RunButton({
  busy,
  disabled,
  label,
  onClick,
}: {
  busy: boolean;
  disabled: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button disabled={disabled} onClick={onClick}>
      {busy ? <CircleNotch className="cg-spin" /> : null}
      {busy ? "Running…" : label}
    </Button>
  );
}
