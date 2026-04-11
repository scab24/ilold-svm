<script lang="ts">
  import { untrack } from 'svelte';
  import { SvelteFlow, SvelteFlowProvider, Background, Controls, useSvelteFlow, type NodeTypes } from '@xyflow/svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';

  import FunctionNode from './nodes/FunctionNode.svelte';
  import BlockNode from './nodes/BlockNode.svelte';
  import SequenceNode from './nodes/SequenceNode.svelte';
  import {
    getNodes,
    getEdges,
    type GraphNodeData,
  } from '$lib/stores/graph.svelte';

  // ── Props — event delegation to parent ──────────────────────
  interface Props {
    onnodetap?: (node: Node<GraphNodeData>, event: MouseEvent) => void;
    onbackgroundtap?: () => void;
    oncontextmenu?: (event: MouseEvent, node: Node<GraphNodeData>) => void;
    onready?: (api: { fitView: (opts?: any) => Promise<boolean> }) => void;
  }

  let { onnodetap, onbackgroundtap, oncontextmenu, onready }: Props = $props();

  // ── Custom node type registry ─────────────────────────────
  const nodeTypes: NodeTypes = {
    function: FunctionNode,
    block: BlockNode,
    sequence: SequenceNode,
  } as unknown as NodeTypes;

  // ── Reactive bridge: graph store → SvelteFlow ──────────────
  // The graph store is the source of truth for graph STRUCTURE (nodes/edges existing).
  // SvelteFlow's bind:nodes mutates flowNodes locally for drag/select POSITIONS.
  //
  // When the store changes (e.g. addNodes for CFG expand), we MERGE:
  //   - For existing nodes: keep the live drag position from flowNodes
  //   - For new nodes: use the store position
  // This preserves drag state across structural mutations.

  let flowNodes = $state<Node[]>([]);
  let flowEdges = $state<Edge[]>([]);

  $effect(() => {
    const storeNodes = getNodes();
    // Read flowNodes WITHOUT tracking — we only react to store changes, not local
    const liveByID = new Map(untrack(() => flowNodes).map(n => [n.id, n]));
    flowNodes = storeNodes.map(sn => {
      const live = liveByID.get(sn.id);
      // If node existed and has been dragged, preserve its visual position
      if (live?.position) {
        return { ...sn, position: live.position } as Node;
      }
      return sn as Node;
    });
  });

  $effect(() => {
    const storeEdges = getEdges();
    flowEdges = storeEdges;
  });

  /** Read the live (drag-aware) position of a node from SvelteFlow's local state */
  export function getLiveNode(id: string): Node | undefined {
    return flowNodes.find(n => n.id === id);
  }
</script>

<!--
  oncontextmenu on the wrapper suppresses the NATIVE browser context menu
  for the whole canvas (pane + nodes + controls). Without this, right-clicking
  a node would fire onnodecontextmenu AND the browser's native menu, which
  immediately dismisses our custom ContextMenu popover.
-->
<div class="graph-canvas-flow" oncontextmenu={(e) => e.preventDefault()}>
  <SvelteFlowProvider>
    <SvelteFlow
      bind:nodes={flowNodes}
      bind:edges={flowEdges}
      {nodeTypes}
      onnodeclick={({ event, node }) => onnodetap?.(node as Node<GraphNodeData>, event as MouseEvent)}
      onpaneclick={() => onbackgroundtap?.()}
      onnodecontextmenu={({ event, node }) => {
        // Belt-and-suspenders: preventDefault on the MouseEvent itself in case
        // a future wrapper refactor drops the container-level handler above.
        (event as MouseEvent).preventDefault();
        oncontextmenu?.(event as MouseEvent, node as Node<GraphNodeData>);
      }}
      oninit={() => {
        const { fitView } = useSvelteFlow();
        onready?.({ fitView });
      }}
      minZoom={0.1}
      maxZoom={4}
      colorMode="dark"
    >
      <Background />
      <Controls />
    </SvelteFlow>
  </SvelteFlowProvider>
</div>

<style>
  .graph-canvas-flow {
    width: 100%;
    height: 100%;
  }
  :global(.svelte-flow .svelte-flow__edge-text) {
    font-size: 11px;
    fill: var(--color-accent-hover);
  }
</style>
