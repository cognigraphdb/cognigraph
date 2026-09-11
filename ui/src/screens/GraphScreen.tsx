import { BracketsCurly, CircleNotch, Graph, Play, Plus } from "@phosphor-icons/react";
import { Button, Form, Input, InputNumber, Segmented, Select } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { CreateEdgeDialog } from "../components/CreateEdgeDialog.tsx";
import { ErrorAlert } from "../components/ErrorAlert.tsx";
import { GraphCanvas } from "../components/GraphCanvas.tsx";
import { GraphInspector } from "../components/GraphInspector.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { SelectedGraphPath } from "../components/SelectedGraphPath.tsx";
import {
  type ExplorerGraph,
  mergeExplorerGraphs,
  parseTraversalResponse,
  pathForSelection,
} from "../lib/graph-explorer.ts";
import type { Notify } from "../types.ts";

interface GraphScreenProps {
  api: CogniGraphApi;
  notify: Notify;
  onOpenDocument: (id: string) => void;
}

export function GraphScreen({ api, notify, onOpenDocument }: GraphScreenProps) {
  const { graphWrite } = useAccess();
  // No mock defaults and no auto-run: the start vertex is the operator's,
  // and the edge collection comes from this tenant's actual catalog.
  const [startVertex, setStartVertex] = useState("");
  const [edgeCollections, setEdgeCollections] = useState<string[]>([]);
  const [edgeCollection, setEdgeCollection] = useState<string>();
  const [direction, setDirection] = useState("outbound");
  const [depth, setDepth] = useState(2);
  const [minConfidence, setMinConfidence] = useState(0.5);
  const [mode, setMode] = useState<"visual" | "json">("visual");
  const [graph, setGraph] = useState<ExplorerGraph>();
  const [rootId, setRootId] = useState("");
  const [selectedNodeId, setSelectedNodeId] = useState<string>();
  const [selectedEdgeId, setSelectedEdgeId] = useState<string>();
  const [selectedPathId, setSelectedPathId] = useState<string>();
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");
  const [edgeDialog, setEdgeDialog] = useState(false);

  const selectedPath = useMemo(
    () => graph?.paths.find((path) => path.id === selectedPathId),
    [graph, selectedPathId],
  );
  const selectedNode = graph?.nodes.find((node) => node.id === selectedNodeId);
  const selectedEdge = graph?.edges.find((edge) => edge.id === selectedEdgeId);

  const loadTraversal = useCallback(
    async (vertexId: string, merge = false, edgeOverride?: string) => {
      setRunning(true);
      setError("");
      try {
        const response = await api.post("/graph/traverse", {
          start_vertex: vertexId,
          edge_collection: edgeOverride ?? edgeCollection,
          direction,
          min_depth: 1,
          max_depth: depth,
          min_confidence: minConfidence,
          path_decay: 0.8,
        });
        const incoming = parseTraversalResponse(response);
        if (merge && incoming.nodes.length === 0) {
          setSelectedNodeId(vertexId);
          setSelectedEdgeId(undefined);
          notify("No additional neighbors found from this node");
          return;
        }
        setGraph((current) =>
          merge ? mergeExplorerGraphs(current, incoming, vertexId) : incoming,
        );
        setSelectedNodeId(vertexId);
        setSelectedEdgeId(undefined);
        setSelectedPathId(incoming.paths[0]?.id);
        notify(merge ? "Neighborhood added to the graph" : "Graph traversal completed");
      } catch (loadError) {
        const message = loadError instanceof Error ? loadError.message : "Traversal failed";
        setError(message);
        notify(message, "error");
      } finally {
        setRunning(false);
      }
    },
    [api, depth, direction, edgeCollection, minConfidence, notify],
  );

  useEffect(() => {
    api
      .get<{ collections: Array<{ name: string; collection_type: string }> }>("/collections")
      .then(({ collections }) => {
        const edges = collections
          .filter((info) => info.collection_type === "edge")
          .map((info) => info.name);
        setEdgeCollections(edges);
        setEdgeCollection((current) => current ?? edges[0]);
      })
      .catch(() => setEdgeCollections([]));
  }, [api]);

  const traverse = () => {
    if (!startVertex.trim() || !edgeCollection) return;
    setRootId(startVertex.trim());
    void loadTraversal(startVertex.trim());
  };

  const selectNode = (id: string) => {
    setSelectedNodeId(id);
    setSelectedEdgeId(undefined);
    if (graph) setSelectedPathId(pathForSelection(graph, id)?.id);
  };

  const selectEdge = (id: string) => {
    const edge = graph?.edges.find((item) => item.id === id);
    setSelectedEdgeId(id);
    setSelectedNodeId(edge?.to);
    if (graph) setSelectedPathId(pathForSelection(graph, undefined, id)?.id);
  };

  // A created edge shows up through the real traversal pipeline: expand the
  // from-vertex (merging when it is already on canvas) over the collection
  // the edge was written to, so it appears regardless of the toolbar state.
  const edgeCreated = (from: string, collection: string) => {
    setEdgeDialog(false);
    setEdgeCollection(collection);
    if (graph?.nodes.some((node) => node.id === from)) {
      void loadTraversal(from, true, collection);
    } else {
      setStartVertex(from);
      setRootId(from);
      void loadTraversal(from, false, collection);
    }
  };

  return (
    <main className="page-workspace graph-explorer-page">
      <PageHeader
        actions={
          <div className="actions-row-base">
            <Button
              icon={<Plus aria-hidden="true" size={17} />}
              disabled={!graphWrite}
              title={!graphWrite ? "Your role cannot create relationships." : undefined}
              onClick={() => setEdgeDialog(true)}
            >
              New relationship
            </Button>
            <ViewSwitch mode={mode} onMode={setMode} />
          </div>
        }
        description="Explore typed relationships and follow multi-hop paths from a document vertex."
        eyebrow="Graph / traversal"
        title="Canvas-first neighborhood explorer"
      />
      <Form className="graph-toolbar" layout="vertical" onFinish={traverse}>
        <Form.Item className="graph-start-field" label="Start vertex" required>
          <Input
            onChange={(event) => setStartVertex(event.target.value)}
            placeholder="collection/key — e.g. labels/000ae256…"
            value={startVertex}
          />
        </Form.Item>
        <Form.Item label="Edge collection" required>
          <Select
            disabled={edgeCollections.length === 0}
            onChange={setEdgeCollection}
            options={edgeCollections.map((name) => ({ label: name, value: name }))}
            placeholder={edgeCollections.length === 0 ? "None in this tenant" : "Select"}
            value={edgeCollection}
          />
        </Form.Item>
        <Form.Item label="Direction">
          <Select
            onChange={setDirection}
            options={[
              { label: "Outbound", value: "outbound" },
              { label: "Inbound", value: "inbound" },
              { label: "Any", value: "any" },
            ]}
            value={direction}
          />
        </Form.Item>
        <Form.Item label="Maximum depth">
          <InputNumber
            controls={false}
            max={8}
            min={1}
            onChange={(value) => setDepth(value ?? 1)}
            value={depth}
          />
        </Form.Item>
        <Form.Item label="Minimum confidence">
          <InputNumber
            controls={false}
            max={1}
            min={0}
            onChange={(value) => setMinConfidence(value ?? 0)}
            step={0.05}
            value={minConfidence}
          />
        </Form.Item>
        <Button
          htmlType="submit"
          icon={
            running ? (
              <CircleNotch className="cg-spin" weight="bold" />
            ) : (
              <Play aria-hidden="true" size={17} weight="fill" />
            )
          }
          disabled={running || !startVertex.trim() || !edgeCollection}
          type="primary"
        >
          {running ? "Running…" : "Run traversal"}
        </Button>
      </Form>
      <section className="graph-explorer-surface">
        {mode === "visual" ? (
          <VisualExplorer
            error={error}
            graph={graph}
            onEdge={selectEdge}
            onNode={selectNode}
            rootId={rootId}
            selectedEdgeId={selectedEdgeId}
            selectedNodeId={selectedNodeId}
            selectedPathId={selectedPathId}
          />
        ) : (
          <div className="graph-json-result">
            <JsonResult empty="Run a traversal to inspect its JSON response." value={graph?.raw} />
          </div>
        )}
        {mode === "visual" && graph ? (
          <GraphInspector
            edge={selectedEdge}
            loading={running}
            node={selectedNode}
            onClose={() => {
              setSelectedNodeId(undefined);
              setSelectedEdgeId(undefined);
            }}
            onExpand={(id) => void loadTraversal(id, true)}
            onOpenDocument={onOpenDocument}
          />
        ) : null}
      </section>
      {mode === "visual" && graph ? (
        <SelectedGraphPath edges={graph.edges} nodes={graph.nodes} path={selectedPath} />
      ) : null}
      {edgeDialog && graphWrite ? (
        <CreateEdgeDialog
          api={api}
          edgeCollections={edgeCollections}
          initialCollection={edgeCollection}
          initialFrom={selectedNodeId ?? (startVertex.trim() || undefined)}
          nodes={graph?.nodes ?? []}
          notify={notify}
          onClose={() => setEdgeDialog(false)}
          onCreated={edgeCreated}
        />
      ) : null}
    </main>
  );
}

function VisualExplorer({
  error,
  graph,
  rootId,
  selectedNodeId,
  selectedEdgeId,
  selectedPathId,
  onNode,
  onEdge,
}: {
  error: string;
  graph?: ExplorerGraph;
  rootId: string;
  selectedNodeId?: string;
  selectedEdgeId?: string;
  selectedPathId?: string;
  onNode: (id: string) => void;
  onEdge: (id: string) => void;
}) {
  if (error) {
    return (
      <div className="empty-state-base graph-empty-state">
        <ErrorAlert title={error} />
      </div>
    );
  }
  if (!graph || graph.nodes.length === 0) {
    return (
      <div className="empty-state-base graph-empty-state">
        <Graph aria-hidden="true" size={42} />
        <span>
          {graph
            ? "No connected paths found."
            : "Enter a start vertex (collection/key), pick an edge collection, and run a traversal."}
        </span>
      </div>
    );
  }
  return (
    <div className="graph-canvas-wrap">
      <GraphCanvas
        graph={graph}
        onEdge={onEdge}
        onNode={onNode}
        rootId={rootId}
        selectedEdgeId={selectedEdgeId}
        selectedNodeId={selectedNodeId}
        selectedPath={graph.paths.find((path) => path.id === selectedPathId)}
      />
    </div>
  );
}

function ViewSwitch({
  mode,
  onMode,
}: {
  mode: "visual" | "json";
  onMode: (mode: "visual" | "json") => void;
}) {
  return (
    <Segmented
      aria-label="Graph view"
      className="graph-view-switch"
      onChange={(value) => onMode(value as "visual" | "json")}
      options={[
        {
          label: (
            <span className="graph-view-option">
              <Graph aria-hidden="true" size={16} /> Visual
            </span>
          ),
          value: "visual",
        },
        {
          label: (
            <span className="graph-view-option">
              <BracketsCurly aria-hidden="true" size={16} /> JSON
            </span>
          ),
          value: "json",
        },
      ]}
      value={mode}
    />
  );
}
