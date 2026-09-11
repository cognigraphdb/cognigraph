import { CircleNotch, ClockCounterClockwise, Play } from "@phosphor-icons/react";
import { Button, Form, Input, Tabs, Tag } from "antd";
import { useCallback, useMemo, useState } from "react";
import { ApiError, type CogniGraphApi } from "../api/client.ts";
import { CgqlEditor } from "../components/CgqlEditor.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { RequestFeedback } from "../components/RequestFeedback.tsx";
import { SearchPanel } from "../components/SearchPanel.tsx";
import { useExecution } from "../hooks/useExecution.ts";
import { useRequestResult } from "../hooks/useRequestResult.ts";
import {
  diagnosticFromCgqlError,
  emptyQueryDiagnostic,
  missingBindVariableDiagnostics,
  parseBindVariables,
  prepareCgqlValidation,
  unavailableDiagnostic,
} from "../lib/cgql-validation.ts";
import { SEARCH_MODE_META, type SearchMode } from "../lib/search.ts";
import type { Notify } from "../types.ts";

const defaultQuery = "FOR d IN documents\n  SORT d.updatedAt DESC\n  LIMIT 25\n  RETURN d";

export function QueryScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const [query, setQuery] = useState(defaultQuery);
  const [bindVars, setBindVars] = useState("{}");
  const { state: result, run: runResult } = useRequestResult<unknown>(
    api,
    JSON.stringify([query, bindVars]),
  );
  const { running, execute } = useExecution(api);
  const elapsed = "elapsed" in result ? result.elapsed : undefined;
  const [history, setHistory] = useState<string[]>([]);
  const parsedBindVars = useMemo(() => parseBindVariables(bindVars), [bindVars]);

  const validate = useCallback(
    async (candidate: string) => {
      if (!candidate.trim()) return [emptyQueryDiagnostic()];
      if ("error" in parsedBindVars) return [];
      const missingBinds = missingBindVariableDiagnostics(candidate, parsedBindVars.value);
      if (missingBinds.length > 0) return missingBinds;
      const prepared = prepareCgqlValidation(candidate);
      try {
        await api.post("/search/query", {
          query: prepared.query,
          bind_vars: parsedBindVars.value,
          language: "cgql",
        });
        return [];
      } catch (error) {
        if (error instanceof ApiError) {
          return [diagnosticFromCgqlError(candidate, error.message, prepared.lineOffset)];
        }
        return [
          unavailableDiagnostic(
            candidate,
            error instanceof Error ? error.message : "server unavailable",
          ),
        ];
      }
    },
    [api, parsedBindVars],
  );

  const runQuery = () =>
    execute(() =>
      runResult(
        async () => {
          if ("error" in parsedBindVars) throw new Error(parsedBindVars.error);
          return api.post("/search/query", {
            query,
            bind_vars: parsedBindVars.value,
            language: "cgql",
          });
        },
        () => {
          setHistory((current) => [query, ...current.filter((item) => item !== query)].slice(0, 5));
          notify("CGQL query completed");
        },
      ),
    );

  const searchTabs = (Object.keys(SEARCH_MODE_META) as SearchMode[]).map((mode) => ({
    key: mode,
    label: SEARCH_MODE_META[mode].label,
    children: <SearchPanel api={api} mode={mode} />,
  }));

  return (
    <main className="page-workspace no-scroll">
      <PageHeader
        description="Read-only CGQL plus the retrieval modes: semantic, hybrid, raw-vector, and graph-augmented search."
        eyebrow="Developer tools / query"
        title="Query console"
      />
      <Tabs
        className="query-mode-tabs"
        defaultActiveKey="cgql"
        destroyOnHidden
        items={[
          {
            key: "cgql",
            label: "CGQL",
            children: (
              <div className="split-console">
                <Form className="console-card query-editor" onFinish={() => void runQuery()}>
                  <div className="card-heading compact-heading">
                    <h2>Query</h2>
                    {elapsed !== undefined ? <Tag>{elapsed} ms</Tag> : null}
                  </div>
                  <CgqlEditor
                    onChange={setQuery}
                    onRun={() => void runQuery()}
                    validate={validate}
                    validationRevision={bindVars}
                    value={query}
                  />
                  <div className="editor-hint">
                    <span>Validated against the active CGQL engine after you pause</span>
                    <kbd>⌘ Enter or Ctrl Enter</kbd>
                  </div>
                  <Form.Item
                    className="bind-vars"
                    help={"error" in parsedBindVars ? parsedBindVars.error : undefined}
                    label="Bind variables"
                    validateStatus={"error" in parsedBindVars ? "error" : undefined}
                  >
                    <Input.TextArea
                      onChange={(event) => setBindVars(event.target.value)}
                      spellCheck={false}
                      status={"error" in parsedBindVars ? "error" : undefined}
                      value={bindVars}
                    />
                  </Form.Item>
                  <Button
                    className="run-button"
                    htmlType="submit"
                    disabled={running}
                    icon={
                      running ? (
                        <CircleNotch className="cg-spin" weight="bold" />
                      ) : (
                        <Play aria-hidden="true" size={17} weight="fill" />
                      )
                    }
                    type="primary"
                  >
                    {running ? "Running…" : "Run query"}
                  </Button>
                  <div className="query-history">
                    <span>
                      <ClockCounterClockwise aria-hidden="true" size={16} /> Recent
                    </span>
                    {history.map((item) => (
                      <Button key={item} onClick={() => setQuery(item)} size="small">
                        {item.split("\n")[0]}
                      </Button>
                    ))}
                  </div>
                </Form>
                <section className="console-card result-panel">
                  <div className="card-heading compact-heading">
                    <h2>Result</h2>
                  </div>
                  <div className="result-body">
                    <RequestFeedback state={result} waiting={running} />
                    {result.status === "success" ? <JsonResult value={result.data} /> : null}
                  </div>
                </section>
              </div>
            ),
          },
          ...searchTabs,
        ]}
      />
    </main>
  );
}
