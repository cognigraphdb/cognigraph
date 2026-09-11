import { ArrowRight, ArrowSquareOut, CircleNotch, FileText, Graph, X } from "@phosphor-icons/react";
import { Button } from "antd";
import type { ExplorerNode, ExplorerRelationship } from "../lib/graph-explorer.ts";

interface GraphInspectorProps {
  node?: ExplorerNode;
  edge?: ExplorerRelationship;
  loading?: boolean;
  onClose: () => void;
  onExpand: (id: string) => void;
  onOpenDocument: (id: string) => void;
}

export function GraphInspector({
  node,
  edge,
  loading,
  onClose,
  onExpand,
  onOpenDocument,
}: GraphInspectorProps) {
  if (!node && !edge) return null;
  const expansionId = edge?.to ?? node?.id;
  return (
    <aside className="graph-inspector">
      <header>
        <h2>{edge ? "Selected relationship" : "Selected node"}</h2>
        <Button
          aria-label="Close graph inspector"
          icon={<X aria-hidden="true" size={18} />}
          onClick={onClose}
          type="text"
        />
      </header>
      {node ? <NodeDetails node={node} onOpen={onOpenDocument} /> : null}
      {edge ? <EdgeDetails edge={edge} /> : null}
      {expansionId ? (
        <Button
          block
          className="graph-expand-button"
          icon={
            loading ? (
              <CircleNotch className="cg-spin" weight="bold" />
            ) : (
              <Graph aria-hidden="true" size={17} />
            )
          }
          disabled={loading}
          onClick={() => onExpand(expansionId)}
          type="primary"
        >
          {loading ? "Expanding…" : "Expand from here"}
        </Button>
      ) : null}
    </aside>
  );
}

function NodeDetails({ node, onOpen }: { node: ExplorerNode; onOpen: (id: string) => void }) {
  const title = String(node.vertex.title ?? node.vertex.name ?? node.vertex._key ?? node.id);
  return (
    <section className="graph-inspector-section">
      <div className="graph-inspector-identity">
        <span className="graph-inspector-icon">
          <FileText aria-hidden="true" size={24} />
        </span>
        <div>
          <strong>{title}</strong>
          <span>{node.vertex._key ?? node.id}</span>
        </div>
      </div>
      <dl className="graph-properties">
        <Property name="Category" value={node.vertex.category} />
        <Property name="Depth" value={node.depth} />
        <Property name="Confidence" value={node.confidence?.toFixed(2)} />
        <Property name="Status" value={node.vertex.status} />
        <Property name="Priority" value={node.vertex.priority} />
        <Property name="Updated" value={node.vertex.updatedAt ?? node.vertex.updated_at} />
      </dl>
      <Button
        block
        className="graph-open-button"
        icon={<ArrowSquareOut aria-hidden="true" size={17} />}
        onClick={() => onOpen(node.id)}
      >
        Open document
      </Button>
    </section>
  );
}

function EdgeDetails({ edge }: { edge: ExplorerRelationship }) {
  return (
    <section className="graph-inspector-section graph-edge-details">
      <div className="relationship-heading">
        <span className="relationship-arrow">
          <ArrowRight aria-hidden="true" size={19} />
        </span>
        <div>
          <strong>{edge.relationType}</strong>
          <span>{String(edge._id ?? edge.id)}</span>
        </div>
      </div>
      <dl className="graph-properties">
        <Property name="Source" value={edge.from} />
        <Property name="Target" value={edge.to} />
        <Property name="Confidence" value={edge.confidence.toFixed(2)} />
        <Property name="Created by" value={edge.created_by ?? edge.source} />
        <Property name="Created" value={edge.created_at} />
      </dl>
    </section>
  );
}

function Property({ name, value }: { name: string; value: unknown }) {
  if (value === undefined || value === null || value === "") return null;
  return (
    <div>
      <dt>{name}</dt>
      <dd>{String(value)}</dd>
    </div>
  );
}
