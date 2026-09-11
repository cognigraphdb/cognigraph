import type { Graph as G6GraphType, IElementEvent, NodeData } from "@antv/g6";
import { EdgeEvent, Graph as G6Graph, NodeEvent } from "@antv/g6/dist/g6.min.js";
import { CornersOut, LockSimple, LockSimpleOpen, Minus, Plus } from "@phosphor-icons/react";
import { type ReactNode, type RefObject, useEffect, useRef, useState } from "react";
import "../styles/graph-canvas.css";
import documentIcon from "../assets/document.svg";
import selectedDocumentIcon from "../assets/document-selected.svg";
import type {
  ExplorerGraph,
  ExplorerNode,
  ExplorerPath,
  ExplorerRelationship,
} from "../lib/graph-explorer.ts";

interface GraphCanvasProps {
  graph: ExplorerGraph;
  rootId: string;
  selectedNodeId?: string;
  selectedEdgeId?: string;
  selectedPath?: ExplorerPath;
  onNode: (id: string) => void;
  onEdge: (id: string) => void;
}

interface G6NodeMeta {
  vertex: ExplorerNode["vertex"];
  depth: number;
  confidence?: number;
  root: boolean;
  selected: boolean;
  path: boolean;
}

interface G6EdgeMeta {
  relationship: ExplorerRelationship;
  selected: boolean;
  path: boolean;
}

const MINIMAP_RENDER_DELAY_MS = 0;
const GRAPH_DESTROY_GRACE_MS = 32;

export function GraphCanvas({
  graph,
  rootId,
  selectedNodeId,
  selectedEdgeId,
  selectedPath,
  onNode,
  onEdge,
}: GraphCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const instanceRef = useRef<G6GraphType | null>(null);
  const initialGraphRef = useRef(graph);
  const renderedRef = useRef(false);
  const onNodeRef = useRef(onNode);
  const onEdgeRef = useRef(onEdge);
  const selectionRef = useRef({ selectedNodeId, selectedEdgeId, selectedPath });
  const lockedRef = useRef(false);
  const [locked, setLocked] = useState(false);
  onNodeRef.current = onNode;
  onEdgeRef.current = onEdge;
  selectionRef.current = { selectedNodeId, selectedEdgeId, selectedPath };
  initialGraphRef.current = graph;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let active = true;
    let instance: G6GraphType | null = null;
    let observer: ResizeObserver | null = null;
    let renderPromise: Promise<void> | null = null;
    const mountTimer = window.setTimeout(() => {
      if (!active) return;
      const selection = selectionRef.current;
      instance = createGraph(
        container,
        initialGraphRef.current,
        rootId,
        selection.selectedNodeId,
        selection.selectedEdgeId,
        selection.selectedPath,
        lockedRef,
      );
      instanceRef.current = instance;
      instance.on(NodeEvent.CLICK, (event) =>
        onNodeRef.current((event as IElementEvent).target.id),
      );
      instance.on(EdgeEvent.CLICK, (event) =>
        onEdgeRef.current((event as IElementEvent).target.id),
      );
      observer = new ResizeObserver(() => instance?.resize());
      observer.observe(container);
      renderPromise = instance
        .render()
        .then(() => {
          if (active) renderedRef.current = true;
        })
        .catch((error: unknown) => {
          if (active) console.error("G6 graph rendering failed", error);
        });
    }, 0);
    return () => {
      active = false;
      renderedRef.current = false;
      window.clearTimeout(mountTimer);
      observer?.disconnect();
      instanceRef.current = null;
      if (instance && renderPromise) {
        const mountedInstance = instance;
        // G6's minimap queues a debounced AFTER_RENDER callback. Destroying the
        // graph in the render promise microtask clears its model before that
        // callback runs, so defer teardown by one short browser frame.
        void renderPromise.then(() => {
          window.setTimeout(() => mountedInstance.destroy(), GRAPH_DESTROY_GRACE_MS);
        });
      }
    };
  }, [rootId]);

  // Data changes replace the model and re-run the layout: G6's updateData
  // only restyles EXISTING elements and throws synchronously on unknown ids,
  // so a merged expansion (or a created edge) that introduces new nodes must
  // go through setData + render. The throw would otherwise unmount the app.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance || !renderedRef.current) return;
    const selection = selectionRef.current;
    try {
      instance.setData(
        mapGraphData(
          graph,
          rootId,
          selection.selectedNodeId,
          selection.selectedEdgeId,
          selection.selectedPath,
        ),
      );
    } catch (error) {
      console.error("G6 graph data replacement failed", error);
      return;
    }
    void instance.render().catch((error: unknown) => {
      console.error("G6 graph re-render failed", error);
    });
  }, [graph, rootId]);

  // Selection changes only restyle elements already in the model.
  useEffect(() => {
    const instance = instanceRef.current;
    if (!instance || !renderedRef.current) return;
    try {
      instance.updateData(
        mapGraphData(initialGraphRef.current, rootId, selectedNodeId, selectedEdgeId, selectedPath),
      );
    } catch (error) {
      console.error("G6 selection update failed", error);
      return;
    }
    void instance.draw().catch((error: unknown) => {
      console.error("G6 graph update failed", error);
    });
  }, [rootId, selectedEdgeId, selectedNodeId, selectedPath]);

  const toggleLock = () => {
    lockedRef.current = !lockedRef.current;
    setLocked(lockedRef.current);
  };

  return (
    <div className="g6-graph-shell">
      <div
        aria-label="Interactive graph visualization"
        className="g6-graph"
        ref={containerRef}
        role="application"
      />
      <div aria-label="Graph controls" className="g6-graph-controls" role="toolbar">
        <Control label="Zoom in" onClick={() => void instanceRef.current?.zoomBy(1.2)}>
          <Plus aria-hidden="true" size={17} />
        </Control>
        <Control label="Zoom out" onClick={() => void instanceRef.current?.zoomBy(0.8)}>
          <Minus aria-hidden="true" size={17} />
        </Control>
        <Control label="Fit graph to view" onClick={() => void instanceRef.current?.fitView()}>
          <CornersOut aria-hidden="true" size={16} />
        </Control>
        <Control
          label={locked ? "Unlock node positions" : "Lock node positions"}
          onClick={toggleLock}
        >
          {locked ? (
            <LockSimple aria-hidden="true" size={15} />
          ) : (
            <LockSimpleOpen aria-hidden="true" size={15} />
          )}
        </Control>
      </div>
    </div>
  );
}

function createGraph(
  container: HTMLElement,
  graph: ExplorerGraph,
  rootId: string,
  selectedNodeId: string | undefined,
  selectedEdgeId: string | undefined,
  selectedPath: ExplorerPath | undefined,
  lockedRef: RefObject<boolean>,
) {
  return new G6Graph({
    container,
    autoFit: "view",
    data: mapGraphData(graph, rootId, selectedNodeId, selectedEdgeId, selectedPath),
    layout: {
      type: "radial",
      focusNode: rootId,
      // No sortBy: it rewrites the same-ring ideal distance to
      // graphDist × |sortValue_i − sortValue_j| — and ring siblings share
      // the same depth, so any depth-like sort collapses a ring's nodes
      // onto ONE point. The default (graphDist × linkDistance) is what
      // spreads siblings around the ring. The overlap pass then budgets
      // for the ~300px labels, not the 32px circles (layout nodeSize).
      unitRadius: 320,
      linkDistance: 320,
      nodeSize: 210,
      nodeSpacing: 40,
      preventOverlap: true,
      strictRadial: true,
    },
    node: {
      type: "circle",
      style: nodeStyle,
    },
    edge: {
      type: "line",
      style: edgeStyle,
    },
    behaviors: [
      "drag-canvas",
      "zoom-canvas",
      { type: "drag-element", key: "drag-node", enable: () => !lockedRef.current },
    ],
    plugins: [
      {
        type: "minimap",
        key: "minimap",
        position: "left-bottom",
        size: [120, 86],
        padding: 8,
        delay: MINIMAP_RENDER_DELAY_MS,
        containerStyle: {
          marginLeft: "12px",
          marginBottom: "12px",
          border: "1px solid #cfd8d6",
          borderRadius: "4px",
          background: "#fff",
        },
      },
    ],
  });
}

function mapGraphData(
  graph: ExplorerGraph,
  rootId: string,
  selectedNodeId?: string,
  selectedEdgeId?: string,
  selectedPath?: ExplorerPath,
) {
  return {
    nodes: graph.nodes.map((node) => {
      return {
        id: node.id,
        data: {
          vertex: node.vertex,
          depth: node.depth,
          confidence: node.confidence,
          root: node.id === rootId,
          selected: node.id === selectedNodeId,
          path: selectedPath?.vertexIds.includes(node.id) ?? false,
        } satisfies G6NodeMeta,
      };
    }),
    edges: graph.edges.map((relationship) => {
      return {
        id: relationship.id,
        source: relationship.from,
        target: relationship.to,
        data: {
          relationship,
          selected: relationship.id === selectedEdgeId,
          path: selectedPath?.edgeIds.includes(relationship.id) ?? false,
        } satisfies G6EdgeMeta,
      };
    }),
  };
}

function nodeStyle(datum: NodeData) {
  const meta = datum.data as unknown as G6NodeMeta;
  const key = String(meta.vertex._key ?? datum.id.split("/").at(-1));
  const title = String(meta.vertex.title ?? meta.vertex.name ?? "Document");
  const confidence = meta.confidence === undefined ? "" : `\n${meta.confidence.toFixed(2)}`;
  const highlighted = meta.root || meta.selected;
  return {
    size: meta.root ? 50 : 32,
    fill: highlighted ? "#087f7c" : "#fff",
    stroke: highlighted ? "#065f5d" : meta.path ? "#087f7c" : "#83918f",
    lineWidth: highlighted ? 2.5 : meta.path ? 2 : 1.2,
    cursor: "pointer" as const,
    icon: true,
    iconSrc: highlighted ? selectedDocumentIcon : documentIcon,
    iconWidth: meta.root ? 22 : 15,
    iconHeight: meta.root ? 22 : 15,
    labelText: `${key}\n${truncate(title, 23)}${confidence}`,
    // Labels hang below every node: right-extending labels on the left
    // half of a ring run inward across their own parent chain.
    labelPlacement: "bottom" as const,
    labelOffsetY: meta.root ? 7 : 4,
    labelFill: "#53605e",
    labelFontSize: 10,
    labelFontFamily: "Inter, ui-sans-serif, system-ui, sans-serif",
    labelFontWeight: meta.root ? 600 : 500,
    labelLineHeight: 13,
    labelTextAlign: "center" as const,
    labelTextBaseline: "top" as const,
  };
}

function edgeStyle(datum: { data?: Record<string, unknown> }) {
  const meta = datum.data as unknown as G6EdgeMeta;
  const edge = meta.relationship;
  const highlighted = meta.selected || meta.path;
  return {
    stroke: highlighted ? "#087f7c" : "#7d8987",
    lineWidth: meta.selected ? 3 : highlighted ? 2.2 : 1.2,
    endArrow: true,
    endArrowSize: 7,
    // Horizontal labels: rotated ones run along steep edges and straight
    // through the neighboring node labels.
    labelAutoRotate: false,
    labelText: `${edge.relationType}  ${edge.confidence.toFixed(2)}`,
    labelFill: highlighted ? "#087f7c" : "#687572",
    labelFontSize: 10,
    labelFontWeight: 500,
    labelBackground: true,
    labelBackgroundFill: "#fff",
    labelBackgroundFillOpacity: 0.9,
    labelPadding: [2, 4] as [number, number],
  };
}

function truncate(value: string, length: number) {
  return value.length > length ? `${value.slice(0, length - 1)}…` : value;
}

function Control({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button aria-label={label} onClick={onClick} title={label} type="button">
      {children}
    </button>
  );
}
