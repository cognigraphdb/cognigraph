import type { JsonObject } from "../types.ts";

export interface ExplorerVertex extends JsonObject {
  _id: string;
  _key?: string;
  title?: string;
  category?: string;
}

export interface ExplorerRelationship extends JsonObject {
  id: string;
  from: string;
  to: string;
  relationType: string;
  confidence: number;
}

export interface ExplorerNode {
  id: string;
  vertex: ExplorerVertex;
  depth: number;
  confidence?: number;
}

export interface ExplorerPath {
  id: string;
  vertexIds: string[];
  edgeIds: string[];
  depth: number;
  score: number;
  confidence: number;
}

export interface ExplorerGraph {
  nodes: ExplorerNode[];
  edges: ExplorerRelationship[];
  paths: ExplorerPath[];
  raw: unknown;
}

export function parseTraversalResponse(value: unknown): ExplorerGraph {
  const payload = asObject(value);
  const results = Array.isArray(payload?.results) ? payload.results : [];
  const nodes = new Map<string, ExplorerNode>();
  const edges = new Map<string, ExplorerRelationship>();
  const paths: ExplorerPath[] = [];

  for (const candidate of results) {
    const path = asObject(candidate);
    if (!path) continue;
    const vertices = Array.isArray(path.vertices)
      ? path.vertices.map(asVertex).filter(Boolean)
      : [];
    const relationships = Array.isArray(path.edges)
      ? path.edges.map(asRelationship).filter(Boolean)
      : [];

    vertices.forEach((vertex, depth) => {
      if (!vertex) return;
      const previous = nodes.get(vertex._id);
      const confidence = depth > 0 ? relationships[depth - 1]?.confidence : undefined;
      nodes.set(vertex._id, {
        id: vertex._id,
        vertex,
        depth: Math.min(previous?.depth ?? depth, depth),
        confidence: previous?.confidence ?? confidence,
      });
    });
    relationships.forEach((relationship) => {
      if (relationship) edges.set(relationship.id, relationship);
    });

    const vertexIds = vertices.flatMap((vertex) => (vertex ? [vertex._id] : []));
    const edgeIds = relationships.flatMap((edge) => (edge ? [edge.id] : []));
    if (vertexIds.length === 0) continue;
    const confidence = relationships.reduce(
      (lowest, edge) => Math.min(lowest, edge?.confidence ?? 1),
      1,
    );
    paths.push({
      id: `${vertexIds.join(">")}|${edgeIds.join(">")}`,
      vertexIds,
      edgeIds,
      depth: numberValue(path.depth, Math.max(0, vertexIds.length - 1)),
      score: numberValue(path.score, 0),
      confidence,
    });
  }

  paths.sort((left, right) => right.score - left.score || right.confidence - left.confidence);
  return { nodes: [...nodes.values()], edges: [...edges.values()], paths, raw: value };
}

export function mergeExplorerGraphs(
  current: ExplorerGraph | undefined,
  incoming: ExplorerGraph,
  anchorId?: string,
): ExplorerGraph {
  if (!current) return incoming;
  const anchorDepth = current.nodes.find((node) => node.id === anchorId)?.depth ?? 0;
  const nodes = new Map(current.nodes.map((node) => [node.id, node]));
  for (const node of incoming.nodes) {
    const previous = nodes.get(node.id);
    nodes.set(node.id, previous ?? { ...node, depth: anchorDepth + node.depth });
  }
  const edges = new Map(current.edges.map((edge) => [edge.id, edge]));
  for (const edge of incoming.edges) edges.set(edge.id, edge);
  const paths = new Map(current.paths.map((path) => [path.id, path]));
  for (const path of incoming.paths) paths.set(path.id, path);
  return {
    nodes: [...nodes.values()],
    edges: [...edges.values()],
    paths: [...paths.values()],
    raw: incoming.raw,
  };
}

export function pathForSelection(
  graph: ExplorerGraph,
  nodeId?: string,
  edgeId?: string,
): ExplorerPath | undefined {
  if (edgeId) return graph.paths.find((path) => path.edgeIds.includes(edgeId));
  if (nodeId) {
    return [...graph.paths]
      .filter((path) => path.vertexIds.includes(nodeId))
      .sort((left, right) => right.depth - left.depth)[0];
  }
  return graph.paths[0];
}

function asVertex(value: unknown): ExplorerVertex | undefined {
  const vertex = asObject(value);
  if (!vertex || typeof vertex._id !== "string") return undefined;
  return vertex as ExplorerVertex;
}

function asRelationship(value: unknown): ExplorerRelationship | undefined {
  const edge = asObject(value);
  if (!edge || typeof edge._from !== "string" || typeof edge._to !== "string") return undefined;
  const relationType = String(edge.relation_type ?? edge.type ?? "related_to");
  return {
    ...edge,
    id: String(edge._id ?? edge._key ?? `${edge._from}>${edge._to}:${relationType}`),
    from: edge._from,
    to: edge._to,
    relationType,
    confidence: numberValue(edge.confidence, 1),
  };
}

function asObject(value: unknown): JsonObject | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as JsonObject)
    : undefined;
}

function numberValue(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}
