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
  // The graph store ($state in graph.svelte.ts) is the source of truth.
  // SvelteFlow needs bind:nodes/bind:edges for internal mutations (drag/select).
  // We sync store → local via $effect. Local drag changes are visual-only
  // (not persisted back to store — acceptable for now).

  let flowNodes = $state<Node[]>([]);
  let flowEdges = $state<Edge[]>([]);

  // Store → local: when store changes (addNode, setNodes, etc.), sync to local
  $effect(() => {
    const storeNodes = getNodes();
    // Always sync — Svelte 5 $effect tracks the $state read inside getNodes()
    flowNodes = storeNodes as Node[];
  });

  $effect(() => {
    const storeEdges = getEdges();
    flowEdges = storeEdges;
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
