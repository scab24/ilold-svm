<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getCfg, getPaths, type CytoscapeGraph } from '$lib/api/rest';

  let error: string | null = $state(null);
  let pathTree: any = $state(null);
  let selectedPath: any = $state(null);
  let selectedNode: any = $state(null);
  let showPaths: boolean = $state(true);

  let cyContainer: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;

  const contractName = $derived(page.params.name);
  const funcName = $derived(page.params.func);

  onMount(async () => {
    if (!contractName || !funcName) return;
    try {
      const [cfg, paths] = await Promise.all([
        getCfg(contractName, funcName),
        getPaths(contractName, funcName),
      ]);
      pathTree = paths;
      await tick();
      await new Promise(r => requestAnimationFrame(r));
      if (cyContainer && cfg) renderCfg(cfg);
    } catch (e) {
      error = `Function "${funcName}" not found in ${contractName}`;
    }
  });

  onDestroy(() => {
    if (cyInstance) { cyInstance.destroy(); cyInstance = null; }
  });

  async function renderCfg(graph: CytoscapeGraph) {
    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    const nodes = graph.nodes.map(n => ({ group: 'nodes' as const, data: n.data }));
    const edges = graph.edges.map(e => ({ group: 'edges' as const, data: e.data }));

    cyInstance = cytoscape({
      container: cyContainer,
      elements: [...nodes, ...edges],
      style: [
        {
          selector: 'node',
          style: {
            'label': 'data(label)', 'color': '#c9d1d9', 'font-size': '10px',
            'text-valign': 'center', 'text-halign': 'center',
            'width': '180px', 'height': '34px', 'shape': 'roundrectangle',
            'background-color': '#161b22', 'border-width': 1, 'border-color': '#30363d',
            'text-max-width': '170px', 'text-wrap': 'ellipsis',
          }
        },
        { selector: 'node[node_type = "Entry"]', style: { 'background-color': '#1f6feb', 'border-color': '#58a6ff', 'color': '#f0f6fc' } },
        { selector: 'node[node_type = "Return"]', style: { 'background-color': '#238636', 'border-color': '#3fb950', 'color': '#f0f6fc', 'width': '100px' } },
        { selector: 'node[node_type = "Revert"]', style: { 'background-color': '#da3633', 'border-color': '#f85149', 'color': '#f0f6fc', 'width': '100px' } },
        { selector: 'node[node_type = "LoopCondition"]', style: { 'background-color': '#9e6a03', 'border-color': '#d29922', 'color': '#f0f6fc', 'shape': 'diamond', 'width': '100px', 'height': '50px' } },
        { selector: 'node[node_type = "Assembly"]', style: { 'background-color': '#6e40c9', 'border-color': '#8b5cf6', 'color': '#f0f6fc' } },
        { selector: 'node.highlighted', style: { 'border-width': 3, 'border-color': '#58a6ff' } },
        { selector: 'node.dimmed', style: { 'opacity': 0.3 } },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        { selector: 'edge', style: { 'width': 1.5, 'line-color': '#30363d', 'target-arrow-color': '#30363d', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.8 } },
        { selector: 'edge[kind ^= "ConditionalTrue"]', style: { 'line-color': '#3fb95088', 'target-arrow-color': '#3fb950', 'label': '✓', 'font-size': '14px', 'color': '#3fb950' } },
        { selector: 'edge[kind ^= "ConditionalFalse"]', style: { 'line-color': '#f8514988', 'target-arrow-color': '#f85149', 'label': '✗', 'font-size': '14px', 'color': '#f85149' } },
        { selector: 'edge[kind = "LoopBack"]', style: { 'line-color': '#d29922', 'target-arrow-color': '#d29922', 'line-style': 'dashed' } },
        { selector: 'edge.highlighted', style: { 'width': 3, 'line-color': '#58a6ff', 'target-arrow-color': '#58a6ff', 'opacity': 1 } },
        { selector: 'edge.dimmed', style: { 'opacity': 0.15 } },
      ],
      layout: { name: 'preset' },
      minZoom: 0.15, maxZoom: 5, wheelSensitivity: 0.3,
    });

    const layout = cyInstance.layout({ name: 'dagre', rankDir: 'TB', nodeSep: 50, rankSep: 60, animate: false } as any);
    layout.run(); layout.stop();

    cyInstance.on('tap', 'node', (evt: any) => {
      selectedNode = evt.target.data();
      cyInstance.elements().removeClass('highlighted dimmed');
      evt.target.addClass('highlighted');
    });

    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) { resetSelection(); }
    });

    cyInstance.on('mouseover', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
  }

  function highlightPath(path: any) {
    if (!cyInstance) return;
    selectedPath = path;
    selectedNode = null;

    cyInstance.elements().addClass('dimmed').removeClass('highlighted');

    const blockIds = path.nodes.map((n: any) => `b${n.block_id}`);
    blockIds.forEach((id: string) => {
      const node = cyInstance.getElementById(id);
      if (node.length) { node.removeClass('dimmed'); node.addClass('highlighted'); }
    });

    for (let i = 0; i < blockIds.length - 1; i++) {
      const edges = cyInstance.edges(`[source = "${blockIds[i]}"][target = "${blockIds[i + 1]}"]`);
      edges.removeClass('dimmed');
      edges.addClass('highlighted');
    }
  }

  function resetSelection() {
    selectedNode = null;
    selectedPath = null;
    if (cyInstance) cyInstance.elements().removeClass('highlighted dimmed');
  }

  function terminalColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : t === 'LoopCutoff' ? '#d29922' : '#8b949e';
  }

  function fitGraph() { if (cyInstance) cyInstance.fit(undefined, 40); }
</script>

<div class="fullscreen-view">
  <!-- Top bar -->
  <div class="topbar">
    <a href="/contract/{contractName}">← {contractName}</a>
    <span class="func-name">{funcName}</span>
    <span class="stats">
      {pathTree?.stats.total_paths ?? '...'} paths
      <span class="happy">{pathTree?.stats.happy_paths ?? 0} ✓</span>
      <span class="revert">{pathTree?.stats.revert_paths ?? 0} ✗</span>
    </span>
    <div class="toolbar">
      <button onclick={fitGraph} title="Fit to screen">⊡</button>
      <button onclick={() => showPaths = !showPaths} title="Toggle paths panel">
        {showPaths ? '▶' : '◀'} Paths
      </button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else}
    <!-- Full screen graph -->
    <div class="canvas" bind:this={cyContainer}></div>

    <!-- Floating legend -->
    <div class="legend">
      <span><span class="dot" style="background:#1f6feb"></span>Entry</span>
      <span><span class="dot" style="background:#161b22;border:1px solid #30363d"></span>Normal</span>
      <span><span class="dot" style="background:#238636"></span>Return</span>
      <span><span class="dot" style="background:#da3633"></span>Revert</span>
      <span style="color:#3fb950">✓ True</span>
      <span style="color:#f85149">✗ False</span>
    </div>

    <!-- Floating node detail -->
    {#if selectedNode}
      <div class="floating-panel node-panel">
        <div class="panel-header">
          <strong>{selectedNode.id}</strong>
          <span class="node-type" style="color:{terminalColor(selectedNode.node_type)}">{selectedNode.node_type}</span>
          <button class="close" onclick={() => resetSelection()}>✕</button>
        </div>
        {#if selectedNode.statements?.length > 0}
          <div class="stmt-list">
            {#each selectedNode.statements as stmt}
              <div class="stmt">{stmt}</div>
            {/each}
          </div>
        {:else}
          <div class="empty">No statements</div>
        {/if}
      </div>
    {/if}

    <!-- Floating paths panel -->
    {#if showPaths && pathTree}
      <div class="floating-panel paths-panel">
        <div class="panel-header">
          <strong>Paths ({pathTree.paths.length})</strong>
          <button class="close" onclick={() => showPaths = false}>✕</button>
        </div>
        <div class="path-list">
          {#each pathTree.paths as path}
            <button
              class="path-row"
              class:selected={selectedPath?.id === path.id}
              onclick={() => highlightPath(path)}
            >
              <span class="path-id">#{path.id}</span>
              <span style="color:{terminalColor(path.terminal)};font-weight:600">{path.terminal}</span>
              <span class="depth">{path.nodes.length}blk</span>
              {#if path.annotations.external_calls.length > 0}
                <span class="badge ext">⚡{path.annotations.external_calls.length}</span>
              {/if}
              {#if path.annotations.state_writes.length > 0}
                <span class="badge write">✏{path.annotations.state_writes.length}</span>
              {/if}
            </button>
          {/each}
        </div>

        {#if selectedPath}
          <div class="path-detail">
            {#if selectedPath.annotations.require_checks.length > 0}
              <div class="detail-group">
                <div class="detail-title">Checks</div>
                {#each selectedPath.annotations.require_checks as c}
                  <div class="detail-item check">{c}</div>
                {/each}
              </div>
            {/if}
            {#if selectedPath.annotations.external_calls.length > 0}
              <div class="detail-group">
                <div class="detail-title">External calls</div>
                {#each selectedPath.annotations.external_calls as c}
                  <div class="detail-item ext">{c.target}.{c.function}()</div>
                {/each}
              </div>
            {/if}
            {#if selectedPath.annotations.state_writes.length > 0}
              <div class="detail-group">
                <div class="detail-title">State writes</div>
                {#each selectedPath.annotations.state_writes as w}
                  <div class="detail-item write">{w}</div>
                {/each}
              </div>
            {/if}
            {#if selectedPath.annotations.events_emitted.length > 0}
              <div class="detail-group">
                <div class="detail-title">Events</div>
                {#each selectedPath.annotations.events_emitted as e}
                  <div class="detail-item event">{e}</div>
                {/each}
              </div>
            {/if}
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
    display: flex;
    flex-direction: column;
    background: #0d1117;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #161b22;
    border-bottom: 1px solid #30363d;
    z-index: 10;
    flex-shrink: 0;
  }

  .topbar a { font-size: 13px; color: #8b949e; }
  .func-name { font-size: 16px; font-weight: 700; color: #f0f6fc; font-family: monospace; }
  .stats { font-size: 12px; color: #8b949e; }
  .stats .happy { color: #3fb950; }
  .stats .revert { color: #f85149; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .toolbar button {
    background: #21262d; border: 1px solid #30363d; color: #c9d1d9;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .toolbar button:hover { border-color: #58a6ff; }

  .error { padding: 24px; color: #f85149; }

  .canvas {
    flex: 1;
    width: 100%;
  }

  .legend {
    position: fixed;
    bottom: 12px; left: 16px;
    display: flex; gap: 10px;
    font-size: 11px; color: #8b949e;
    background: #161b22cc;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid #30363d;
    z-index: 10;
  }

  .dot {
    display: inline-block;
    width: 8px; height: 8px;
    border-radius: 2px;
    vertical-align: middle;
    margin-right: 3px;
  }

  .floating-panel {
    position: fixed;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    z-index: 20;
    box-shadow: 0 8px 24px #00000066;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
  }

  .panel-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid #21262d;
    font-size: 13px;
    color: #f0f6fc;
    position: sticky;
    top: 0;
    background: #161b22;
  }

  .close {
    margin-left: auto;
    background: none; border: none;
    color: #8b949e; cursor: pointer; font-size: 14px;
    padding: 2px 6px; border-radius: 4px;
  }
  .close:hover { background: #21262d; color: #f0f6fc; }

  .node-panel {
    top: 60px; right: 16px;
    width: 300px;
  }

  .node-type { font-size: 11px; }

  .stmt-list { padding: 8px; }
  .stmt {
    font-family: monospace;
    font-size: 12px;
    padding: 4px 8px;
    background: #0d1117;
    border-radius: 4px;
    margin-bottom: 3px;
    color: #c9d1d9;
  }

  .empty { padding: 8px 12px; font-size: 12px; color: #484f58; }

  .paths-panel {
    top: 60px; right: 16px;
    width: 320px;
  }

  .path-list { padding: 4px; }

  .path-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    cursor: pointer;
    color: inherit;
    font: inherit;
    font-size: 12px;
    width: 100%;
    text-align: left;
  }

  .path-row:hover { background: #21262d; }
  .path-row.selected { background: #21262d; border-color: #58a6ff; }
  .path-id { color: #484f58; font-weight: 600; min-width: 24px; }
  .depth { color: #484f58; font-size: 11px; }

  .badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 8px;
  }
  .badge.ext { background: #f851491a; color: #f85149; }
  .badge.write { background: #58a6ff1a; color: #58a6ff; }

  .path-detail {
    padding: 8px;
    border-top: 1px solid #21262d;
  }

  .detail-group { margin-bottom: 8px; }
  .detail-title { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 3px; padding: 0 4px; }

  .detail-item {
    font-family: monospace;
    font-size: 11px;
    padding: 3px 6px;
    border-radius: 3px;
    margin-bottom: 2px;
  }
  .detail-item.check { background: #d299221a; color: #d29922; }
  .detail-item.ext { background: #f851491a; color: #f85149; }
  .detail-item.write { background: #58a6ff1a; color: #58a6ff; }
  .detail-item.event { background: #3fb9501a; color: #3fb950; }
</style>
