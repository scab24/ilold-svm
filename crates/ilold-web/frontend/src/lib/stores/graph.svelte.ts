import type { Node, Edge } from '@xyflow/svelte';

// ── Node data types ─────────────────────────────────────────

export interface FunctionNodeData {
  [key: string]: unknown;
  _type: 'function';
  label: string;
  is_external: boolean;
  contractName?: string;
  _dimmed?: boolean;
}

export interface BlockNodeData {
  [key: string]: unknown;
  _type: 'block';
  label: string;
  node_type: string; // "Entry" | "Return" | "Revert" | "Block" | "LoopCondition"
  _parentFunc: string;
  statements?: string[];
  _dimmed?: boolean;
}

export interface SequenceNodeData {
  [key: string]: unknown;
  _type: 'seq-next';
  label: string;
  _funcName: string;
  _seqParent: string;
  _isBranch: boolean;
  pathCount?: number;
  readOnly?: boolean;
  _transition?: any;
  _chainTransitions?: any[];
  _dimmed?: boolean;
}

export type GraphNodeData = FunctionNodeData | BlockNodeData | SequenceNodeData;

// ── Reactive state ──────────────────────────────────────────
// SvelteFlow uses $bindable nodes/edges — the wrapper component
// binds directly to these arrays via getNodes()/getEdges() and
// setNodes()/setEdges(). SvelteFlow mutates them internally for
// drag, selection, etc.

let nodes = $state<Node<GraphNodeData>[]>([]);
let edges = $state<Edge[]>([]);

// ── Getters ─────────────────────────────────────────────────

export function getNodes(): Node<GraphNodeData>[] {
  return nodes;
}

export function getEdges(): Edge[] {
  return edges;
}

// ── Mutations ───────────────────────────────────────────────

export function setNodes(newNodes: Node<GraphNodeData>[]) {
  nodes = newNodes;
}

export function setEdges(newEdges: Edge[]) {
  edges = newEdges;
}

export function addNode(node: Node<GraphNodeData>) {
  nodes = [...nodes, node];
}

export function addEdge(edge: Edge) {
  edges = [...edges, edge];
}

export function addNodes(newNodes: Node<GraphNodeData>[]) {
  nodes = [...nodes, ...newNodes];
}

export function addEdges(newEdges: Edge[]) {
  edges = [...edges, ...newEdges];
}

export function removeNodeById(id: string) {
  nodes = nodes.filter((n) => n.id !== id);
  edges = edges.filter((e) => e.source !== id && e.target !== id);
}

export function removeNodesById(ids: Set<string>) {
  nodes = nodes.filter((n) => !ids.has(n.id));
  edges = edges.filter((e) => !ids.has(e.source) && !ids.has(e.target));
}

export function updateNode(
  id: string,
  updater: (node: Node<GraphNodeData>) => Node<GraphNodeData>,
) {
  nodes = nodes.map((n) => (n.id === id ? updater(n) : n));
}

export function updateNodeData(id: string, data: Partial<GraphNodeData>) {
  nodes = nodes.map((n) =>
    n.id === id ? { ...n, data: { ...n.data, ...data } as GraphNodeData } : n,
  );
}

export function clearGraph() {
  nodes = [];
  edges = [];
}

// ── Queries ─────────────────────────────────────────────────

export function findNode(id: string): Node<GraphNodeData> | undefined {
  return nodes.find((n) => n.id === id);
}

export function findNodesByType(
  type: GraphNodeData['_type'],
): Node<GraphNodeData>[] {
  return nodes.filter((n) => n.data._type === type);
}

export function findNodesByParentFunc(
  funcName: string,
): Node<GraphNodeData>[] {
  return nodes.filter(
    (n) => '_parentFunc' in n.data && n.data._parentFunc === funcName,
  );
}

export function findDescendants(parentId: string): Set<string> {
  const descendants = new Set<string>();
  const queue = [parentId];

  while (queue.length > 0) {
    const current = queue.shift()!;

    for (const node of nodes) {
      if (
        '_seqParent' in node.data &&
        node.data._seqParent === current &&
        !descendants.has(node.id)
      ) {
        descendants.add(node.id);
        queue.push(node.id);
      }
    }
  }

  return descendants;
}
