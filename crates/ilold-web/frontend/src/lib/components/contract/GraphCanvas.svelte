<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { CytoscapeGraph } from '$lib/api/rest';

  interface Props {
    graphData: CytoscapeGraph | null;
    expandedFuncs: Set<string>;
    onnodetap: (nodeData: any) => void;
    onbackgroundtap: () => void;
    onnodecontextmenu: (x: number, y: number, nodeId: string, funcName: string, nodeType: string) => void;
    onfunctiontap: (funcName: string, nodeId: string, shiftKey: boolean) => void;
    onseqnodetap: (funcName: string, nodeId: string, shiftKey: boolean, isBranch: boolean, seqParent: string) => void;
  }

  let {
    graphData,
    expandedFuncs,
    onnodetap,
    onbackgroundtap,
    onnodecontextmenu,
    onfunctiontap,
    onseqnodetap,
  }: Props = $props();

  let cyContainer: HTMLDivElement;
  let canvasWrap: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;

  /** Expose the Cytoscape instance for parent mutations */
  export function getCy(): any { return cyInstance; }

  /** Expose container ref for coordinate calculations */
  export function getContainer(): HTMLDivElement { return cyContainer; }

  // Palette: dark board
  const C = {
    bg: '#121215',
    surface: '#1a1a22',
    border: '#252530',
    borderHi: '#4a6fa5',
    text: '#b8c4d4',
    textMuted: '#6b7a8d',
    accent: '#5b9bd5',
    accentDark: '#3a6b9f',
    accentLight: '#8bb8e8',
    warn: '#c49a4a',
    warnBorder: '#8a6d30',
    danger: '#b05050',
    dangerLight: '#c07070',
    ok: '#5a9a6a',
    dangerBg: '#3a2020',
    edge: '#363a48',
    edgeHi: '#5b9bd5',
  };

  function getStyles() {
    return [
      // Function nodes
      {
        selector: 'node.internal',
        style: {
          'background-color': C.surface, 'label': 'data(label)', 'color': C.accentLight,
          'font-size': '12px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '150px', 'height': '40px', 'shape': 'roundrectangle',
          'border-width': 1.5, 'border-color': C.accent,
        }
      },
      {
        selector: 'node.external',
        style: {
          'background-color': C.bg, 'label': 'data(label)', 'color': C.dangerLight,
          'font-size': '11px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '130px', 'height': '34px', 'shape': 'roundrectangle',
          'border-style': 'dashed', 'border-width': 1, 'border-color': C.danger,
        }
      },
      // CFG block nodes
      {
        selector: 'node.block',
        style: {
          'label': 'data(label)', 'color': C.text, 'font-size': '9px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '160px', 'height': '30px', 'shape': 'roundrectangle',
          'background-color': C.surface, 'border-width': 1, 'border-color': C.border,
          'text-max-width': '150px', 'text-wrap': 'ellipsis',
        }
      },
      { selector: 'node.block-entry', style: { 'background-color': C.accentDark, 'border-color': C.accent, 'color': '#dce8f4' } },
      { selector: 'node.block-return', style: { 'background-color': '#2a4a35', 'border-color': C.ok, 'color': '#b8d4c4', 'width': '90px' } },
      { selector: 'node.block-revert', style: { 'background-color': C.dangerBg, 'border-color': C.danger, 'color': C.dangerLight, 'width': '90px' } },
      { selector: 'node.block-loopcondition', style: { 'background-color': '#38301e', 'border-color': C.warn, 'color': '#d4c49a', 'shape': 'diamond', 'width': '90px', 'height': '45px' } },
      { selector: 'node:active', style: { 'overlay-opacity': 0 } },
      // Call edges
      {
        selector: 'edge[_type = "call"]',
        style: {
          'width': 1, 'line-color': C.edge, 'target-arrow-color': C.edge,
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.7,
        }
      },
      {
        selector: 'edge[kind = "External"]',
        style: { 'line-color': '#b0505044', 'target-arrow-color': C.danger, 'line-style': 'dashed' }
      },
      // CFG edges
      {
        selector: 'edge[_type = "cfg-edge"]',
        style: {
          'width': 1, 'line-color': C.border, 'target-arrow-color': C.border,
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.6,
        }
      },
      { selector: 'edge.cond-true', style: { 'line-color': '#5a9a6a66', 'target-arrow-color': C.ok, 'label': '✓', 'font-size': '11px', 'color': C.ok } },
      { selector: 'edge.cond-false', style: { 'line-color': '#b0505066', 'target-arrow-color': C.danger, 'label': '✗', 'font-size': '11px', 'color': C.danger } },
      { selector: 'edge.loop-back', style: { 'line-color': C.warn, 'target-arrow-color': C.warn, 'line-style': 'dashed' } },
      { selector: 'edge.expand-link', style: { 'line-color': '#5b9bd544', 'target-arrow-color': C.accent, 'line-style': 'dotted', 'width': 2 } },
      // Sequence nodes
      {
        selector: 'node.seq-next',
        style: {
          'label': 'data(label)', 'color': C.accentLight, 'font-size': '10px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '110px', 'height': '28px', 'shape': 'roundrectangle',
          'background-color': C.surface, 'border-width': 1.5, 'border-color': C.accent,
        }
      },
      { selector: 'node.seq-next.readonly', style: { 'border-color': C.textMuted, 'color': C.textMuted } },
      { selector: 'node.seq-next.has-conditions', style: { 'background-color': '#2e2818', 'border-color': C.warn, 'border-width': 2, 'color': '#d4c49a' } },
      { selector: 'node.seq-next.has-shared', style: { 'border-style': 'dashed' } },
      {
        selector: 'edge.seq-edge',
        style: {
          'width': 1.5, 'line-color': C.edge, 'target-arrow-color': C.textMuted,
          'target-arrow-shape': 'triangle', 'curve-style': 'straight', 'arrow-scale': 0.7,
        }
      },
      {
        selector: 'edge.seq-cond',
        style: {
          'line-color': C.warn, 'target-arrow-color': C.warn, 'width': 2,
          'label': 'data(label)', 'font-size': '14px', 'color': C.warn,
        }
      },
    ];
  }

  /** Collect all descendants of a node recursively via _seqParent */
  function collectAllDescendants(rootId: string) {
    let result = cyInstance.collection();
    const direct = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === rootId);
    direct.forEach((c: any) => {
      result = result.union(c);
      result = result.union(collectAllDescendants(c.id()));
    });
    return result;
  }

  /** Grid follows zoom and pan */
  function updateGrid() {
    if (!canvasWrap || !cyInstance) return;
    const zoom = cyInstance.zoom();
    const pan = cyInstance.pan();
    const size = 24 * zoom;
    canvasWrap.style.setProperty('--grid-size', `${size}px`);
    canvasWrap.style.setProperty('--grid-x', `${pan.x % size}px`);
    canvasWrap.style.setProperty('--grid-y', `${pan.y % size}px`);
  }

  async function init(graph: CytoscapeGraph) {
    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    // Start with empty canvas — functions are added from sidebar
    cyInstance = cytoscape({
      container: cyContainer,
      elements: [],
      style: getStyles() as any,
      layout: { name: 'preset' },
      minZoom: 0.1, maxZoom: 5, wheelSensitivity: 0.3,
    });

    // Single click on function nodes (internal)
    cyInstance.on('tap', 'node.internal', async (evt: any) => {
      const data = evt.target.data();
      if (data._type !== 'function') return;
      onfunctiontap(data.label, data.id, evt.originalEvent?.shiftKey ?? false);
    });

    // Single click on seq-next nodes
    cyInstance.on('tap', 'node.seq-next', async (evt: any) => {
      const data = evt.target.data();
      const funcName = data._funcName || data.label;
      const nodeId = data.id;
      onseqnodetap(funcName, nodeId, evt.originalEvent?.shiftKey ?? false, !!data._isBranch, data._seqParent);
    });

    // Click any node -> show info panel
    cyInstance.on('tap', 'node', async (evt: any) => {
      const data = evt.target.data();
      onnodetap(data);
    });

    // Click background -> deselect
    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) {
        onbackgroundtap();
      }
    });

    // Drag node -> move children together
    cyInstance.on('drag', 'node', (evt: any) => {
      const node = evt.target;
      const prevX = node.data('_prevX');
      const prevY = node.data('_prevY');
      if (prevX === undefined || prevY === undefined) return;

      const delta = { x: evt.position.x - prevX, y: evt.position.y - prevY };
      node.data('_prevX', evt.position.x);
      node.data('_prevY', evt.position.y);

      const nodeId = node.id();
      const nodeType = node.data('_type');
      let children = cyInstance.collection();

      if (nodeType === 'function') {
        const funcName = node.data('label');
        if (expandedFuncs.has(funcName)) {
          children = children.union(cyInstance.nodes(`[_parentFunc = "${funcName}"]`));
        }
        children = children.union(collectAllDescendants(nodeId));
      } else if (nodeType === 'seq-next') {
        children = collectAllDescendants(nodeId);
      }

      children.forEach((child: any) => {
        const pos = child.position();
        child.position({ x: pos.x + delta.x, y: pos.y + delta.y });
      });
    });

    cyInstance.on('grab', 'node', (evt: any) => {
      const pos = evt.target.position();
      evt.target.data('_prevX', pos.x);
      evt.target.data('_prevY', pos.y);
    });

    cyInstance.on('mouseover', 'node.internal', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseover', 'node.seq-next', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });

    // Right-click -> context menu
    cyInstance.on('cxttap', 'node', (evt: any) => {
      evt.originalEvent?.preventDefault();
      const data = evt.target.data();
      const rect = cyContainer.getBoundingClientRect();
      const pos = evt.renderedPosition || evt.position;
      onnodecontextmenu(
        pos.x + rect.left,
        pos.y + rect.top,
        data.id,
        data._type === 'function' ? data.label : (data._parentFunc || data._funcName || data.label),
        data._type,
      );
    });

    // Grid follows zoom and pan
    cyInstance.on('zoom pan', updateGrid);
    updateGrid();
  }

  function destroy() {
    if (cyInstance) {
      cyInstance.destroy();
      cyInstance = null;
    }
  }

  onMount(async () => {
    if (graphData) await init(graphData);
  });

  $effect(() => {
    // Re-init when graphData changes (but not on first mount — onMount handles that)
    if (graphData && cyContainer && !cyInstance) {
      init(graphData);
    }
  });

  onDestroy(() => {
    destroy();
  });
</script>

<div class="canvas-wrap" bind:this={canvasWrap}>
  <div class="canvas" bind:this={cyContainer} oncontextmenu={(e) => e.preventDefault()}></div>
</div>

<style>
  /* Canvas with dot grid that follows zoom/pan */
  .canvas-wrap {
    flex: 1; position: relative;
    --grid-size: 24px; --grid-x: 0px; --grid-y: 0px;
  }
  .canvas-wrap::before {
    content: '';
    position: absolute; inset: 0; z-index: 0; pointer-events: none;
    background-image: radial-gradient(circle, #333340 1px, transparent 1px);
    background-size: var(--grid-size) var(--grid-size);
    background-position: var(--grid-x) var(--grid-y);
  }
  .canvas { position: absolute; inset: 0; z-index: 1; }
</style>
