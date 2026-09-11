import { CircleNotch, Play } from "@phosphor-icons/react";
import { Button, Tag } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { LuaEditor } from "../components/LuaEditor.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import type { Notify } from "../types.ts";

interface LuaScreenProps {
  api: CogniGraphApi;
  notify: Notify;
}

const defaultScript = `-- Sandboxed Lua with the graph API: no filesystem, network, or OS.
-- graph.query() speaks the backend's language (CGQL on native).
return { backend = graph.backend, language = graph.query_language }`;

/// Canned starting points; each replaces the editor content.
const snippets: Array<{ label: string; script: string }> = [
  {
    label: "Backend info",
    script: defaultScript,
  },
  {
    label: "Find documents",
    script: `-- First 5 documents of a collection.
return graph.find_documents("documents", { limit = 5 })`,
  },
  {
    label: "Run a query",
    script: `-- graph.query() runs CGQL on the native backend.
return graph.query("FOR d IN documents LIMIT 3 RETURN { key: d._key, title: d.title }")`,
  },
  {
    label: "Traverse",
    script: `-- Multi-hop neighborhood from a vertex.
return graph.traverse("documents/some-key", {
  edge_collection = "document_relations",
  direction = "outbound",
  max_depth = 2,
})`,
  },
];

export function LuaScreen({ api, notify }: LuaScreenProps) {
  const { luaWrite } = useAccess();
  const [script, setScript] = useState(defaultScript);
  const [result, setResult] = useState<unknown>();
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState<number>();

  const run = async () => {
    setRunning(true);
    const started = performance.now();
    try {
      const response = await api.post("/lua/execute", { script });
      setResult(response);
      notify("Script completed");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Script failed";
      setResult({ error: message });
      notify(message, "error");
    } finally {
      setElapsed(Math.round(performance.now() - started));
      setRunning(false);
    }
  };

  return (
    <main className="page-workspace no-scroll">
      <PageHeader
        description={
          luaWrite
            ? "Run sandboxed Lua with read and write access to your tenant. Execution is instruction-limited; filesystem, network and OS access are unavailable."
            : "Run read-only sandboxed Lua against your tenant. This session cannot mutate data through Lua. Execution is instruction-limited."
        }
        eyebrow="Developer tools / scripting"
        title="Lua console"
      />
      <div className="split-console">
        <section className="console-card query-editor">
          <div className="card-heading compact-heading">
            <h2>Script</h2>
            {elapsed !== undefined ? <Tag>{elapsed} ms</Tag> : null}
          </div>
          <LuaEditor onChange={setScript} onRun={() => void run()} value={script} />
          <div className="editor-hint">
            <span>graph API: query · find_documents · traverse · neighbors · …</span>
            <kbd>⌘ Enter or Ctrl Enter</kbd>
          </div>
          <Button
            className="run-button"
            disabled={running}
            icon={
              running ? (
                <CircleNotch className="cg-spin" weight="bold" />
              ) : (
                <Play aria-hidden="true" size={17} weight="fill" />
              )
            }
            onClick={() => void run()}
            type="primary"
          >
            {running ? "Running…" : "Run script"}
          </Button>
          <div className="query-history">
            <span>Snippets</span>
            {snippets.map((snippet) => (
              <Button key={snippet.label} onClick={() => setScript(snippet.script)} size="small">
                {snippet.label}
              </Button>
            ))}
          </div>
        </section>
        <section className="console-card result-panel">
          <div className="card-heading compact-heading">
            <h2>Result</h2>
          </div>
          <div className="result-body">
            <JsonResult empty="Run a script to inspect its result." value={result} />
          </div>
        </section>
      </div>
    </main>
  );
}
