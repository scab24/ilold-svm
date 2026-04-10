import dagre from '@dagrejs/dagre';
import type { Node, Edge } from '@xyflow/svelte';
import type { GraphNodeData } from '$lib/stores/graph.svelte';

export interface LayoutOptions {
  rankDir?: 'TB' | 'LR';
  nodeSep?: number;
  rankSep?: number;
  nodeWidth?: number;
  nodeHeight?: number;
}

/**
 * Run dagre layout on nodes/edges, returning a new array with
 * updated positions. Pure function — does not mutate inputs.
 */
export function runDagreLayout(
  nodes: Node<GraphNodeData>[],
  edges: Edge[],
  options: LayoutOptions = {},
): Node<GraphNodeData>[] {
  const {
    rankDir = 'TB',
    nodeSep = 30,
    rankSep = 45,
    nodeWidth = 150,
    nodeHeight = 40,
  } = options;

  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: rankDir, nodesep: nodeSep, ranksep: rankSep });

  for (const node of nodes) {
    g.setNode(node.id, { width: nodeWidth, height: nodeHeight });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  return nodes.map((node) => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: { x: pos.x - nodeWidth / 2, y: pos.y - nodeHeight / 2 },
    };
  });
}

// Map Cytoscape node _type to Svelte Flow node type (custom component name)
function nodeTypeFromData(data: any): string {
  if (data._type === 'block') return 'block';
  if (data._type === 'seq-next') return 'sequence';
  return 'function';
}

function mapNodeData(data: any): GraphNodeData {
  const type = data._type ?? 'function';
  if (type === 'block') {
    return {
      _type: 'block',
      label: data.label ?? '',
      node_type: data.node_type ?? 'Block',
      _parentFunc: data._parentFunc ?? '',
      statements: data.statements,
    };
  }
  if (type === 'seq-next') {
    return {
      _type: 'seq-next',
      label: data.label ?? '',
      _funcName: data._funcName ?? data.label ?? '',
      _seqParent: data._seqParent ?? '',
      _isBranch: data._isBranch ?? false,
      pathCount: data.pathCount,
      readOnly: data.readOnly,
      _transition: data._transition,
      _chainTransitions: data._chainTransitions,
    };
  }
  return {
    _type: 'function',
    label: data.label ?? '',
    is_external: data.is_external ?? false,
    contractName: data.contractName,
  };
}

/**
 * Convert CytoscapeGraph API response to Svelte Flow nodes/edges.
 * Handles all 3 node types (function, block, seq-next) and preserves data fields.
 */
export function cytoscapeToFlow(graphData: {
  nodes: any[];
  edges: any[];
}): { nodes: Node<GraphNodeData>[]; edges: Edge[] } {
  const nodes: Node<GraphNodeData>[] = graphData.nodes.map((n) => ({
    id: n.data.id,
    type: nodeTypeFromData(n.data),
    position: n.position ?? { x: 0, y: 0 },
    data: mapNodeData(n.data),
  }));

  const edges: Edge[] = graphData.edges.map((e) => ({
    id: e.data.id,
    source: e.data.source,
    target: e.data.target,
    type: 'default',
    data: { _type: e.data._type ?? 'call', kind: e.data.kind },
  }));

  return { nodes, edges };
}
