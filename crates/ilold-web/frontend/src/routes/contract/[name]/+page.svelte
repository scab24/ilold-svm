<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getContract, getCallGraph, type ContractDetail, type CytoscapeGraph } from '$lib/api/rest';

  let contract: ContractDetail | null = $state(null);
  let error: string | null = $state(null);
  let showFunctions: boolean = $state(true);
  let selectedNode: any = $state(null);

  let cyContainer: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    try {
      contract = await getContract(contractName);
      const callgraph = await getCallGraph(contractName);
      await tick();
      await new Promise(r => requestAnimationFrame(r));
      if (cyContainer && callgraph) renderCallGraph(callgraph);
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  onDestroy(() => {
    if (cyInstance) { cyInstance.destroy(); cyInstance = null; }
  });

  async function renderCallGraph(graph: CytoscapeGraph) {
    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    const nodes = graph.nodes
      .filter(n => n.data.label.length > 0)
      .map(n => ({ group: 'nodes' as const, data: n.data }));
    const nodeIds = new Set(nodes.map(n => n.data.id));
    const edges = graph.edges
      .filter(e => nodeIds.has(e.data.source) && nodeIds.has(e.data.target))
      .map(e => ({ group: 'edges' as const, data: e.data }));

    cyInstance = cytoscape({
      container: cyContainer,
      elements: [...nodes, ...edges],
      style: [
        {
          selector: 'node[node_type = "internal"]',
          style: {
            'background-color': '#238636', 'label': 'data(label)', 'color': '#f0f6fc',
            'font-size': '12px', 'text-valign': 'center', 'text-halign': 'center',
            'width': '140px', 'height': '36px', 'shape': 'roundrectangle',
          }
        },
        {
          selector: 'node[node_type = "external"]',
          style: {
            'background-color': '#161b22', 'label': 'data(label)', 'color': '#f85149',
            'font-size': '11px', 'text-valign': 'center', 'text-halign': 'center',
            'width': '130px', 'height': '32px', 'shape': 'roundrectangle',
            'border-style': 'dashed', 'border-width': 1, 'border-color': '#f85149',
          }
        },
        { selector: 'node.highlighted', style: { 'border-width': 3, 'border-color': '#58a6ff' } },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        {
          selector: 'edge',
          style: {
            'width': 1.5, 'line-color': '#30363d', 'target-arrow-color': '#30363d',
            'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.8,
          }
        },
        {
          selector: 'edge[kind = "External"]',
          style: { 'line-color': '#f8514966', 'target-arrow-color': '#f85149', 'line-style': 'dashed' }
        },
      ],
      layout: { name: 'preset' },
      minZoom: 0.2, maxZoom: 4, wheelSensitivity: 0.3,
    });

    const layout = cyInstance.layout({ name: 'dagre', rankDir: 'TB', nodeSep: 60, rankSep: 70, animate: false } as any);
    layout.run(); layout.stop();

    cyInstance.on('tap', 'node[node_type = "internal"]', (evt: any) => {
      const label = evt.target.data('label');
      if (label && contract) {
        window.location.href = `/contract/${contract.name}/${label}`;
      }
    });

    cyInstance.on('tap', 'node', (evt: any) => {
      selectedNode = evt.target.data();
      cyInstance.elements().removeClass('highlighted');
      evt.target.addClass('highlighted');
    });

    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) {
        selectedNode = null;
        cyInstance.elements().removeClass('highlighted');
      }
    });

    cyInstance.on('mouseover', 'node[node_type = "internal"]', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
  }

  function fitGraph() { if (cyInstance) cyInstance.fit(undefined, 40); }
</script>

<div class="fullscreen-view">
  <div class="topbar">
    <a href="/">← Contracts</a>
    <span class="contract-kind">{contract?.kind.toLowerCase() ?? ''}</span>
    <span class="contract-name">{contract?.name ?? 'Loading...'}</span>
    {#if contract?.inherits.length}
      <span class="inherits">inherits {contract.inherits.join(', ')}</span>
    {/if}
    <div class="toolbar">
      <button onclick={fitGraph} title="Fit to screen">⊡</button>
      <button onclick={() => showFunctions = !showFunctions}>
        {showFunctions ? '▶' : '◀'} Functions
      </button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else}
    <div class="canvas" bind:this={cyContainer}></div>

    <div class="legend">
      <span><span class="dot" style="background:#238636"></span>Internal</span>
      <span><span class="dot" style="background:#161b22;border:1px solid #f85149"></span>External</span>
      <span>Click function → view paths</span>
    </div>

    {#if selectedNode}
      <div class="floating-panel info-panel">
        <div class="panel-header">
          <strong>{selectedNode.label}</strong>
          <span class="badge" class:ext={selectedNode.is_external} class:int={!selectedNode.is_external}>
            {selectedNode.is_external ? 'external' : 'internal'}
          </span>
          <button class="close" onclick={() => { selectedNode = null; cyInstance?.elements().removeClass('highlighted'); }}>✕</button>
        </div>
        <div class="info-body">
          <div class="info-row">Contract: <strong>{selectedNode.contract}</strong></div>
          {#if !selectedNode.is_external}
            <a href="/contract/{contract?.name}/{selectedNode.label}" class="view-paths-btn">View paths →</a>
          {/if}
        </div>
      </div>
    {/if}

    {#if showFunctions && contract}
      <div class="floating-panel functions-panel">
        <div class="panel-header">
          <strong>Functions ({contract.functions.length})</strong>
          <button class="close" onclick={() => showFunctions = false}>✕</button>
        </div>
        <div class="func-list">
          {#each contract.functions as func}
            <a href="/contract/{contract.name}/{func.name || 'constructor'}" class="func-row">
              <span class="vis">{func.visibility.toLowerCase()}</span>
              <span class="fname">{func.name || 'constructor'}</span>
              <span class="fstats">
                {func.path_count}p
                {#if func.happy_paths > 0}<span class="happy">{func.happy_paths}✓</span>{/if}
                {#if func.revert_paths > 0}<span class="revert">{func.revert_paths}✗</span>{/if}
              </span>
            </a>
          {/each}
        </div>

        {#if contract.state_vars.length > 0}
          <div class="panel-header" style="margin-top:4px">
            <strong>State Variables ({contract.state_vars.length})</strong>
          </div>
          <div class="var-list">
            {#each contract.state_vars as sv}
              <div class="var-row">
                <span class="var-name">{sv.name}</span>
                <span class="var-type">{sv.type_name}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .fullscreen-view {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    display: flex; flex-direction: column;
    background: #0d1117;
  }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px;
    background: #161b22;
    border-bottom: 1px solid #30363d;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .contract-kind { font-size: 12px; color: #8b949e; }
  .contract-name { font-size: 16px; font-weight: 700; color: #f0f6fc; }
  .inherits { font-size: 11px; color: #484f58; font-style: italic; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .toolbar button {
    background: #21262d; border: 1px solid #30363d; color: #c9d1d9;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .toolbar button:hover { border-color: #58a6ff; }

  .error { padding: 24px; color: #f85149; }

  .canvas { flex: 1; width: 100%; }

  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px;
    font-size: 11px; color: #8b949e;
    background: #161b22cc; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #30363d;
    z-index: 10;
  }
  .dot {
    display: inline-block; width: 8px; height: 8px;
    border-radius: 2px; vertical-align: middle; margin-right: 3px;
  }

  .floating-panel {
    position: fixed;
    background: #161b22; border: 1px solid #30363d;
    border-radius: 8px; z-index: 20;
    box-shadow: 0 8px 24px #00000066;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
  }
  .panel-header {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 12px; border-bottom: 1px solid #21262d;
    font-size: 13px; color: #f0f6fc;
    position: sticky; top: 0; background: #161b22;
  }
  .close {
    margin-left: auto; background: none; border: none;
    color: #8b949e; cursor: pointer; font-size: 14px;
    padding: 2px 6px; border-radius: 4px;
  }
  .close:hover { background: #21262d; color: #f0f6fc; }

  .info-panel { top: 60px; right: 16px; width: 280px; }
  .info-body { padding: 10px 12px; }
  .info-row { font-size: 12px; color: #8b949e; margin-bottom: 6px; }
  .info-row strong { color: #c9d1d9; }
  .badge { font-size: 10px; padding: 2px 6px; border-radius: 8px; }
  .badge.ext { background: #f851491a; color: #f85149; }
  .badge.int { background: #2386361a; color: #3fb950; }
  .view-paths-btn {
    display: inline-block; padding: 6px 12px;
    background: #21262d; border: 1px solid #30363d;
    border-radius: 6px; font-size: 12px; color: #58a6ff;
  }
  .view-paths-btn:hover { border-color: #58a6ff; text-decoration: none; }

  .functions-panel { top: 60px; right: 16px; width: 340px; }

  .func-list, .var-list { padding: 4px; }

  .func-row {
    display: flex; align-items: center; gap: 6px;
    padding: 6px 8px; border-radius: 4px;
    color: inherit; font-size: 12px;
  }
  .func-row:hover { background: #21262d; text-decoration: none; }
  .vis { color: #484f58; min-width: 50px; font-size: 11px; }
  .fname { color: #f0f6fc; font-weight: 600; flex: 1; }
  .fstats { color: #8b949e; font-size: 11px; display: flex; gap: 4px; }
  .fstats .happy { color: #3fb950; }
  .fstats .revert { color: #f85149; }

  .var-row {
    display: flex; justify-content: space-between; align-items: baseline;
    padding: 4px 8px; font-size: 11px; font-family: monospace;
    border-bottom: 1px solid #21262d;
  }
  .var-row:last-child { border-bottom: none; }
  .var-name { color: #c9d1d9; font-weight: 600; }
  .var-type { color: #484f58; font-size: 10px; text-align: right; max-width: 150px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
