<script lang="ts">
  import { SvelteFlow, SvelteFlowProvider, Background, Controls, useSvelteFlow, type NodeTypes } from '@xyflow/svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';

  import FunctionNode from './nodes/FunctionNode.svelte';
  import BlockNode from './nodes/BlockNode.svelte';
  import SequenceNode from './nodes/SequenceNode.svelte';
  import {
    getNodes,
    getEdges,
    setNodes,
    setEdges,
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

  // ── Reactive bridge: graph store ↔ SvelteFlow ──────────────
  //
  // SvelteFlow uses bind:nodes/bind:edges (Svelte 5 $bindable).
  // The graph store is the source of truth for external mutations
  // (addNode, removeNode, setNodes, etc.).
  //
  // Direction 1 — Store → SvelteFlow: $derived reads from store,
  //   but bind: requires a writable variable, not $derived.
  //   So we use $state + $effect to sync store → local.
  //
  // Direction 2 — SvelteFlow → Store: bind: writes to local.
  //   We sync back to store in $effect, using reference equality
  //   to break cycles (store set → triggers store→local effect →
  //   sets same reference → local→store effect sees same ref → no-op).

  let flowNodes = $state<Node[]>(getNodes());
  let flowEdges = $state<Edge[]>(getEdges());

  // Store → local (external mutations like addNode/setNodes)
  $effect(() => {
    const latest = getNodes();
    if (latest !== flowNodes) flowNodes = latest;
  });

  $effect(() => {
    const latest = getEdges();
    if (latest !== flowEdges) flowEdges = latest;
  });

  // Local → store (SvelteFlow internal mutations like drag/select)
  $effect(() => {
    if (flowNodes !== getNodes()) setNodes(flowNodes as Node<GraphNodeData>[]);
  });

  $effect(() => {
    if (flowEdges !== getEdges()) setEdges(flowEdges);
  });
</script>

<div class="graph-canvas-flow">
  <SvelteFlowProvider>
    <SvelteFlow
      bind:nodes={flowNodes}
      bind:edges={flowEdges}
      {nodeTypes}
      onnodeclick={({ event, node }) => onnodetap?.(node as Node<GraphNodeData>, event as MouseEvent)}
      onpaneclick={() => onbackgroundtap?.()}
      onnodecontextmenu={({ event, node }) => oncontextmenu?.(event, node as Node<GraphNodeData>)}
      oninit={() => {
        const { fitView } = useSvelteFlow();
        onready?.({ fitView });
      }}
      fitView
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
  :global(.svelte-flow .svelte-flow__node.expanding) {
    transition: transform 0.3s ease-out;
  }
  :global(.svelte-flow .svelte-flow__edge-text) {
    font-size: 11px;
    fill: #8bb8e8;
  }
</style>
