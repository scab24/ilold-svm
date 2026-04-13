<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { SvelteFlow, SvelteFlowProvider, Background, Controls, Panel, useSvelteFlow, type NodeTypes } from '@xyflow/svelte';
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

  // ── Canvas mode toggle (Excalidraw/Figma-style) ────────────
  // `pan`     → left-drag pans the viewport; lasso disabled.
  // `select`  → left-drag draws a rectangular lasso; middle/right still pan.
  // In both modes: nodes draggable, right-click context menu, shift+click multi-select.
  let canvasMode = $state<'pan' | 'select'>('pan');
  let panOnDragProp = $derived<boolean | number[]>(canvasMode === 'pan' ? true : [1, 2]);
  let selectionOnDragProp = $derived(canvasMode === 'select');

  /**
   * Keyboard shortcuts for mode switching.
   * Excludes edits inside text inputs / textareas / contenteditable — the
   * EmbeddedTerminal captures keystrokes via xterm.js and does not interfere.
   */
  function handleKeydown(e: KeyboardEvent) {
    const t = e.target;
    if (
      t instanceof HTMLInputElement ||
      t instanceof HTMLTextAreaElement ||
      t instanceof HTMLSelectElement ||
      (t instanceof HTMLElement && t.isContentEditable)
    ) {
      return;
    }
    if (e.key === 'v' || e.key === 'V') {
      canvasMode = 'select';
    } else if (e.key === 'h' || e.key === 'H') {
      canvasMode = 'pan';
    } else if (e.key === 'Escape') {
      canvasMode = 'pan';
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<!--
  oncontextmenu on the wrapper suppresses the NATIVE browser context menu
  for the whole canvas (pane + nodes + controls). Without this, right-clicking
  a node would fire onnodecontextmenu AND the browser's native menu, which
  immediately dismisses our custom ContextMenu popover.
-->
<div class="graph-canvas-flow" oncontextmenu={(e) => e.preventDefault()}>
  <SvelteFlowProvider>
    <!--
      Canvas modes (toggled via the top-left Panel toolbar or V/H shortcuts):
      - `pan`    → left-drag pans; lasso disabled.
      - `select` → left-drag draws a rectangular lasso (Excalidraw-style);
                   middle/right-drag still pans.
      Shared in both modes:
      - shift+click on a node    → add/remove from selection (multiSelectionKeyCode)
      - drag any selected node   → move the whole group together (native)
      - partial overlap counts   → lasso only needs to touch a node (selectionMode)
      - right-click on a node    → branch/context menu (onnodecontextmenu)
    -->
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
      panOnDrag={panOnDragProp}
      selectionOnDrag={selectionOnDragProp}
      selectionMode="partial"
      multiSelectionKeyCode="Shift"
    >
      <Background />
      <Controls />
      <Panel position="top-left">
        <div class="bg-surface border border-border rounded-sm p-1 flex gap-1 shadow-[0_4px_16px_var(--color-shadow)]">
          <button
            type="button"
            title="Pan mode (H)"
            aria-label="Pan mode"
            aria-pressed={canvasMode === 'pan'}
            onclick={() => (canvasMode = 'pan')}
            class="flex items-center justify-center w-7 h-7 rounded-sm border-none cursor-pointer transition-colors {canvasMode === 'pan' ? 'bg-accent text-surface' : 'bg-surface-alt text-text-muted hover:bg-hover hover:text-accent-hover'}"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M18 11V6a2 2 0 0 0-4 0v5" />
              <path d="M14 10V4a2 2 0 0 0-4 0v7" />
              <path d="M10 10.5V6a2 2 0 0 0-4 0v9" />
              <path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2a8 8 0 0 1-7.92-6.96c-.2-1.58-.1-2.56 1.05-3.25a2 2 0 0 1 2.87 1.21" />
            </svg>
          </button>
          <button
            type="button"
            title="Select mode (V)"
            aria-label="Select mode"
            aria-pressed={canvasMode === 'select'}
            onclick={() => (canvasMode = 'select')}
            class="flex items-center justify-center w-7 h-7 rounded-sm border-none cursor-pointer transition-colors {canvasMode === 'select' ? 'bg-accent text-surface' : 'bg-surface-alt text-text-muted hover:bg-hover hover:text-accent-hover'}"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M3 3l7.5 18 2.25-7.5L20.25 11.25 3 3z" />
              <path d="M13 13l6 6" />
            </svg>
          </button>
        </div>
      </Panel>
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
