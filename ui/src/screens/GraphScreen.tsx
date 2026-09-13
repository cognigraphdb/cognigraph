import { BracketsCurly, CircleNotch, Graph, Play, Plus } from "@phosphor-icons/react";
import { Button, Form, Input, InputNumber, Segmented, Select } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { CreateEdgeDialog } from "../components/CreateEdgeDialog.tsx";
import { GraphCanvas } from "../components/GraphCanvas.tsx";
import { GraphInspector } from "../components/GraphInspector.tsx";
import { JsonResult } from "../components/JsonResult.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { RequestFeedback } from "../components/RequestFeedback.tsx";
import { SelectedGraphPath } from "../components/SelectedGraphPath.tsx";
import { useRequestResult } from "../hooks/useRequestResult.ts";
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
  const inputKey = JSON.stringify([startVertex, edgeCollection, direction, depth, minConfidence]);
  const { state: result, run: runResult } = useRequestResult<{
    graph: ExplorerGraph;
    rootId: string;
    vertexId: string;
    emptyExpansion: boolean;
  }>(api, inputKey);
  const graph = result.status === "success" ? result.data.graph : undefined;
  const rootId = result.status === "success" ? result.data.rootId : "";
  const [selectedNodeId, setSelectedNodeId] = useState<string>();
  const [selectedEdgeId, setSelectedEdgeId] = useState<string>();
  const [selectedPathId, setSelectedPathId] = useState<string>();
  const running = result.status === "pending";
  const [edgeDialog, setEdgeDialog] = useState(false);
  const [createdTraversal, setCreatedTraversal] = useState<{ vertexId: string }>();

  const selectedPath = useMemo(
    () => graph?.paths.find((path) => path.id === selectedPathId),
    [graph, selectedPathId],
  );
  const selectedNode = graph?.nodes.find((node) => node.id === selectedNodeId);
  const selectedEdge = graph?.edges.find((edge) => edge.id === selectedEdgeId);

  const loadTraversal = useCallback(
    (vertexId: string, merge = false) =>
      runResult(
        async () => {
          const response = await api.post("/graph/traverse", {
            start_vertex: vertexId,
            edge_collection: edgeCollection,
            direction,
            min_depth: 1,
            max_depth: depth,
            min_confidence: minConfidence,
            path_decay: 0.8,
          });
          const incoming = parseTraversalResponse(response);
          return {
            graph: merge ? mergeExplorerGraphs(graph, incoming, vertexId) : incoming,
            rootId: merge ? rootId : vertexId,
            vertexId,
            emptyExpansion: merge && incoming.nodes.length === 0,
          };
        },
        (completed) => {
          setSelectedNodeId(vertexId);
          setSelectedEdgeId(undefined);
          setSelectedPathId(pathForSelection(completed.graph, vertexId)?.id);
          notify(
            completed.emptyExpansion
              ? "No additional neighbors found from this node"
              : merge
                ? "Neighborhood added to the graph"
                : "Graph traversal completed",
          );
        },
      ),
    [api, depth, direction, edgeCollection, minConfidence, notify, graph, rootId, runResult],
  );

  // A newly created relationship may change the toolbar's input scope. Start
  // its readback only after those inputs own the new result store.
  useEffect(() => {
    if (!createdTraversal) return;
    setCreatedTraversal(undefined);
    void loadTraversal(createdTraversal.vertexId);
  }, [createdTraversal, loadTraversal]);

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
    if (collection === edgeCollection && graph?.nodes.some((node) => node.id === from)) {
      void loadTraversal(from, true);
    } else {
      setStartVertex(from);
      setEdgeCollection(collection);
      setCreatedTraversal({ vertexId: from });
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
        <RequestFeedback
          state={result}
          idle="Enter a start vertex and run a traversal. Results clear when traversal inputs change."
        />
        {result.status !== "success" ? null : mode === "visual" ? (
          <VisualExplorer
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
            <p className="dialog-hint">
              Latest traversal response from {result.data.vertexId}. Expanded neighborhoods are
              combined in Visual view.
            </p>
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
  graph,
  rootId,
  selectedNodeId,
  selectedEdgeId,
  selectedPathId,
  onNode,
  onEdge,
}: {
  graph?: ExplorerGraph;
  rootId: string;
  selectedNodeId?: string;
  selectedEdgeId?: string;
  selectedPathId?: string;
  onNode: (id: string) => void;
  onEdge: (id: string) => void;
}) {
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
