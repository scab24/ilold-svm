<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getCfg, getPaths, type CytoscapeGraph } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';
  import DraggablePanel from '$lib/DraggablePanel.svelte';

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
  const pathParam = $derived(page.url.searchParams.get('path'));

  let loadedFunc = '';

  onMount(() => {
    loadFunction();
  });

  // React to URL changes (different function or different path param)
  $effect(() => {
    if (funcName && contractName) {
      if (loadedFunc !== `${contractName}::${funcName}`) {
        loadFunction();
      } else if (pathParam !== null && pathTree) {
        autoSelectPath();
      }
    }
  });

  async function loadFunction() {
    if (!contractName || !funcName) return;
    loadedFunc = `${contractName}::${funcName}`;
    setSearchContext(contractName);
    error = null;
    pathTree = null;
    selectedPath = null;
    selectedNode = null;

    try {
      const [cfg, paths] = await Promise.all([
        getCfg(contractName, funcName),
        getPaths(contractName, funcName),
      ]);
      pathTree = paths;
      await tick();
      await new Promise(r => requestAnimationFrame(r));
      if (cyContainer && cfg) {
        renderCfg(cfg);
        if (pathParam !== null) {
          setTimeout(() => autoSelectPath(), 200);
        }
      }
    } catch (e) {
      error = `Function "${funcName}" not found in ${contractName}`;
    }
  }

  function autoSelectPath() {
    if (!pathTree || pathParam === null) return;
    const pathId = parseInt(pathParam);
    const targetPath = pathTree.paths.find((p: any) => p.id === pathId);
    if (targetPath) highlightPath(targetPath);
  }

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
            'label': 'data(label)', 'color': '#b8c4d4', 'font-size': '10px',
            'text-valign': 'center', 'text-halign': 'center',
            'width': '180px', 'height': '34px', 'shape': 'roundrectangle',
            'background-color': '#1e2028', 'border-width': 1, 'border-color': '#2a2d38',
            'text-max-width': '170px', 'text-wrap': 'ellipsis',
          }
        },
        { selector: 'node[node_type = "Entry"]', style: { 'background-color': '#3a6b9f', 'border-color': '#5b9bd5', 'color': '#b8c4d4' } },
        { selector: 'node[node_type = "Return"]', style: { 'background-color': '#5a9a6a', 'border-color': '#5a9a6a', 'color': '#b8c4d4', 'width': '100px' } },
        { selector: 'node[node_type = "Revert"]', style: { 'background-color': '#b05050', 'border-color': '#c07070', 'color': '#b8c4d4', 'width': '100px' } },
        { selector: 'node[node_type = "LoopCondition"]', style: { 'background-color': '#8a6d30', 'border-color': '#c49a4a', 'color': '#b8c4d4', 'shape': 'diamond', 'width': '100px', 'height': '50px' } },
        { selector: 'node[node_type = "Assembly"]', style: { 'background-color': '#6e40c9', 'border-color': '#8b5cf6', 'color': '#b8c4d4' } },
        { selector: 'node.highlighted', style: { 'border-width': 3, 'border-color': '#5b9bd5' } },
        { selector: 'node.dimmed', style: { 'opacity': 0.3 } },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        { selector: 'edge', style: { 'width': 1.5, 'line-color': '#2a2d38', 'target-arrow-color': '#2a2d38', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.8 } },
        { selector: 'edge[kind ^= "ConditionalTrue"]', style: { 'line-color': '#5a9a6a88', 'target-arrow-color': '#5a9a6a', 'label': '✓', 'font-size': '14px', 'color': '#5a9a6a' } },
        { selector: 'edge[kind ^= "ConditionalFalse"]', style: { 'line-color': '#c0707088', 'target-arrow-color': '#c07070', 'label': '✗', 'font-size': '14px', 'color': '#c07070' } },
        { selector: 'edge[kind = "LoopBack"]', style: { 'line-color': '#c49a4a', 'target-arrow-color': '#c49a4a', 'line-style': 'dashed' } },
        { selector: 'edge.highlighted', style: { 'width': 3, 'line-color': '#5b9bd5', 'target-arrow-color': '#5b9bd5', 'opacity': 1 } },
        { selector: 'edge.dimmed', style: { 'opacity': 0.15 } },
      ],
      layout: { name: 'preset' },
      minZoom: 0.15, maxZoom: 5, wheelSensitivity: 0.3,
    });

    const layout = cyInstance.layout({ name: 'dagre', rankDir: 'TB', nodeSep: 50, rankSep: 60, animate: false } as any);
    layout.run(); layout.stop();

    cyInstance.on('tap', 'node', (evt: any) => {
      const data = evt.target.data();
      // Enrich terminal nodes with incoming edge info
      const incomingEdges = evt.target.incomers('edge');
      const incomingConditions: string[] = [];
      incomingEdges.forEach((edge: any) => {
        const kind = edge.data('kind') || '';
        if (kind.includes('ConditionalFalse')) {
          const cond = kind.match(/condition: "(.+?)"/)?.[1] || '';
          incomingConditions.push(`Failed: ${cond}`);
        } else if (kind.includes('ConditionalTrue')) {
          const cond = kind.match(/condition: "(.+?)"/)?.[1] || '';
          incomingConditions.push(`Passed: ${cond}`);
        } else if (kind.includes('Unconditional')) {
          const sourceNode = edge.source();
          const sourceStmts = sourceNode.data('statements') || [];
          if (sourceStmts.length > 0) {
            incomingConditions.push(`After: ${sourceStmts[sourceStmts.length - 1]}`);
          }
        }
      });
      data._conditions = incomingConditions;
      selectedNode = data;
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
    return t === 'Return' ? '#5a9a6a' : t === 'Revert' ? '#c07070' : t === 'LoopCutoff' ? '#c49a4a' : '#6b7a8d';
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
      <button class="tool-link" onclick={toggleSearch}>🔍</button>
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
      <span><span class="dot" style="background:#3a6b9f"></span>Entry</span>
      <span><span class="dot" style="background:#1e2028;border:1px solid #2a2d38"></span>Normal</span>
      <span><span class="dot" style="background:#5a9a6a"></span>Return</span>
      <span><span class="dot" style="background:#b05050"></span>Revert</span>
      <span style="color:#5a9a6a">✓ True</span>
      <span style="color:#c07070">✗ False</span>
    </div>

    {#if selectedNode}
      <DraggablePanel title="Block {selectedNode.id}" x={12} y={window.innerHeight - 280} width={340} onclose={() => resetSelection()}>
        <div class="node-content">
          <span class="node-type" style="color:{terminalColor(selectedNode.node_type)}">{selectedNode.node_type}</span>

          {#if selectedNode._conditions?.length > 0}
            <div class="conditions">
              {#each selectedNode._conditions as cond}
                <div class="condition-item">{cond}</div>
              {/each}
            </div>
          {/if}

          {#if selectedNode.statements?.length > 0}
            <div class="stmt-label">Statements</div>
            <div class="stmt-list">
              {#each selectedNode.statements as stmt}
                <div class="stmt">{stmt}</div>
              {/each}
            </div>
          {:else if selectedNode.node_type === 'Revert'}
            <div class="terminal-info revert-info">Transaction reverts here. The require/assert condition above failed.</div>
          {:else if selectedNode.node_type === 'Return'}
            <div class="terminal-info return-info">Function returns successfully from this point.</div>
          {/if}
        </div>
      </DraggablePanel>
    {/if}

    {#if showPaths && pathTree}
      <DraggablePanel title="Paths ({pathTree.paths.length})" x={window.innerWidth - 360} y={60} width={340} onclose={() => showPaths = false}>
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
      </DraggablePanel>
    {/if}
  {/if}
</div>

<style>
  .fullscreen-view {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    display: flex;
    flex-direction: column;
    background: #181a20;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #1e2028;
    border-bottom: 1px solid #2a2d38;
    z-index: 10;
    flex-shrink: 0;
  }

  .topbar a { font-size: 13px; color: #6b7a8d; }
  .func-name { font-size: 16px; font-weight: 700; color: #b8c4d4; font-family: monospace; }
  .stats { font-size: 12px; color: #6b7a8d; }
  .stats .happy { color: #5a9a6a; }
  .stats .revert { color: #c07070; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .toolbar button {
    background: #252830; border: 1px solid #2a2d38; color: #b8c4d4;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .toolbar button:hover { border-color: #5b9bd5; }
  .tool-link {
    background: #252830; border: 1px solid #2a2d38; color: #b8c4d4;
    padding: 4px 10px; border-radius: 4px; font-size: 12px;
  }
  .tool-link:hover { border-color: #5b9bd5; text-decoration: none; }

  .error { padding: 24px; color: #c07070; }

  .canvas {
    flex: 1;
    width: 100%;
  }

  .legend {
    position: fixed;
    bottom: 12px; left: 16px;
    display: flex; gap: 10px;
    font-size: 11px; color: #6b7a8d;
    background: #1e2028cc;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid #2a2d38;
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
    background: #1e2028;
    border: 1px solid #2a2d38;
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
    border-bottom: 1px solid #252830;
    font-size: 13px;
    color: #b8c4d4;
    position: sticky;
    top: 0;
    background: #1e2028;
  }

  .close {
    margin-left: auto;
    background: none; border: none;
    color: #6b7a8d; cursor: pointer; font-size: 14px;
    padding: 2px 6px; border-radius: 4px;
  }
  .close:hover { background: #252830; color: #b8c4d4; }

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
    background: #181a20;
    border-radius: 4px;
    margin-bottom: 3px;
    color: #b8c4d4;
  }

  .conditions {
    padding: 6px 8px;
    border-bottom: 1px solid #252830;
  }
  .condition-item {
    font-family: monospace;
    font-size: 11px;
    padding: 3px 6px;
    background: #181a20;
    border-radius: 3px;
    margin-bottom: 2px;
    color: #c49a4a;
  }
  .stmt-label {
    padding: 4px 8px 0;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #4a5568;
  }
  .terminal-info {
    padding: 8px;
    font-size: 11px;
    border-radius: 4px;
    margin: 6px 8px;
  }
  .revert-info { background: #c070701a; color: #c07070; }
  .return-info { background: #5a9a6a1a; color: #5a9a6a; }
  .empty { padding: 8px 12px; font-size: 12px; color: #4a5568; }

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

  .path-row:hover { background: #252830; }
  .path-row.selected { background: #252830; border-color: #5b9bd5; }
  .path-id { color: #4a5568; font-weight: 600; min-width: 24px; }
  .depth { color: #4a5568; font-size: 11px; }

  .badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 8px;
  }
  .badge.ext { background: #c070701a; color: #c07070; }
  .badge.write { background: #5b9bd51a; color: #5b9bd5; }

  .path-detail {
    padding: 8px;
    border-top: 1px solid #252830;
  }

  .detail-group { margin-bottom: 8px; }
  .detail-title { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 3px; padding: 0 4px; }

  .detail-item {
    font-family: monospace;
    font-size: 11px;
    padding: 3px 6px;
    border-radius: 3px;
    margin-bottom: 2px;
  }
  .detail-item.check { background: #c49a4a1a; color: #c49a4a; }
  .detail-item.ext { background: #c070701a; color: #c07070; }
  .detail-item.write { background: #5b9bd51a; color: #5b9bd5; }
  .detail-item.event { background: #5a9a6a1a; color: #5a9a6a; }
</style>
