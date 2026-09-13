import { ArrowRight, CaretDown } from "@phosphor-icons/react";
import type { ExplorerNode, ExplorerPath, ExplorerRelationship } from "../lib/graph-explorer.ts";

interface SelectedGraphPathProps {
  path?: ExplorerPath;
  nodes: ExplorerNode[];
  edges: ExplorerRelationship[];
}

export function SelectedGraphPath({ path, nodes, edges }: SelectedGraphPathProps) {
  if (!path) return null;
  const nodeMap = new Map(nodes.map((node) => [node.id, node]));
  const edgeMap = new Map(edges.map((edge) => [edge.id, edge]));
  return (
    <section className="selected-graph-path">
      <header>
        <strong>
          Selected path <span>(depth {path.depth})</span>
        </strong>
        <div className="path-metrics">
          <span>Score: {path.score.toFixed(2)}</span>
          <span>Confidence: {path.confidence.toFixed(2)}</span>
          <span>Path weight: {path.score.toFixed(2)}</span>
          <CaretDown aria-hidden="true" size={16} />
        </div>
      </header>
      <div className="path-hops">
        {path.vertexIds.map((vertexId, index) => {
          const node = nodeMap.get(vertexId);
          const edge = index > 0 ? edgeMap.get(path.edgeIds[index - 1] ?? "") : undefined;
          return (
            <div className="path-hop-group" key={vertexId}>
              {edge ? (
                <div className="path-edge">
                  <span>{edge.relationType}</span>
                  <ArrowRight aria-hidden="true" size={22} />
                  <small>{edge.confidence.toFixed(2)}</small>
                </div>
              ) : null}
              <div className="path-node">
                <strong>{node?.vertex._key ?? vertexId.split("/").at(-1)}</strong>
                <span>{String(node?.vertex.title ?? node?.vertex.category ?? "Document")}</span>
                <small>Depth {index}</small>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
