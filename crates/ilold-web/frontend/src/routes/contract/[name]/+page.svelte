<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getContract, getCallGraph, getPaths, getCfg, type ContractDetail, type CytoscapeGraph } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';
  import DraggablePanel from '$lib/DraggablePanel.svelte';

  let contract: ContractDetail | null = $state(null);
  let error: string | null = $state(null);
  let selectedFunc: any = $state(null);
  let funcPaths: Record<string, any> = $state({});
  let expandedCfg: string | null = $state(null);
  let cfgInstance: any = null;

  let cyContainer: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;

  let cfgContainer: HTMLDivElement;

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    setSearchContext(contractName);
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
    if (cfgInstance) { cfgInstance.destroy(); cfgInstance = null; }
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
            'width': '150px', 'height': '40px', 'shape': 'roundrectangle',
          }
        },
        {
          selector: 'node[node_type = "external"]',
          style: {
            'background-color': '#161b22', 'label': 'data(label)', 'color': '#f85149',
            'font-size': '11px', 'text-valign': 'center', 'text-halign': 'center',
            'width': '130px', 'height': '34px', 'shape': 'roundrectangle',
            'border-style': 'dashed', 'border-width': 1, 'border-color': '#f85149',
          }
        },
        { selector: 'node.selected', style: { 'border-width': 3, 'border-color': '#58a6ff' } },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        {
          selector: 'edge',
          style: {
            'width': 1.5, 'line-color': '#484f58', 'target-arrow-color': '#484f58',
            'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.8,
          }
        },
        {
          selector: 'edge[kind = "External"]',
          style: { 'line-color': '#f8514966', 'target-arrow-color': '#f85149', 'line-style': 'dashed' }
        },
      ],
      layout: { name: 'preset' },
      minZoom: 0.15, maxZoom: 4, wheelSensitivity: 0.3,
    });

    const layout = cyInstance.layout({ name: 'dagre', rankDir: 'TB', nodeSep: 60, rankSep: 80, animate: false } as any);
    layout.run(); layout.stop();

    // Click function → show detail panel + load paths
    cyInstance.on('tap', 'node', async (evt: any) => {
      const data = evt.target.data();
      cyInstance.elements().removeClass('selected');
      evt.target.addClass('selected');

      const funcName = data.label;
      if (!funcName || !contract) return;

      const func = contract.functions.find(f => f.name === funcName);
      selectedFunc = { ...data, ...(func || {}) };

      if (!funcPaths[funcName]) {
        try {
          funcPaths[funcName] = await getPaths(contract.name, funcName);
          funcPaths = { ...funcPaths };
        } catch {}
      }
    });

    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) {
        selectedFunc = null;
        expandedCfg = null;
        if (cfgInstance) { cfgInstance.destroy(); cfgInstance = null; }
        cyInstance.elements().removeClass('selected');
      }
    });

    cyInstance.on('mouseover', 'node[node_type = "internal"]', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
  }

  async function showCfg(funcName: string) {
    if (!contract) return;
    expandedCfg = funcName;

    const cfg = await getCfg(contract.name, funcName);
    await tick();
    if (!cfgContainer) return;

    const cytoscape = (await import('cytoscape')).default;
    if (cfgInstance) cfgInstance.destroy();

    const nodes = cfg.nodes.map(n => ({ group: 'nodes' as const, data: n.data }));
    const edges = cfg.edges.map(e => ({ group: 'edges' as const, data: e.data }));

    cfgInstance = cytoscape({
      container: cfgContainer,
      elements: [...nodes, ...edges],
      style: [
        {
          selector: 'node', style: {
            'label': 'data(label)', 'color': '#c9d1d9', 'font-size': '9px',
            'text-valign': 'center', 'text-halign': 'center',
            'width': '160px', 'height': '30px', 'shape': 'roundrectangle',
            'background-color': '#21262d', 'border-width': 1, 'border-color': '#30363d',
            'text-max-width': '150px', 'text-wrap': 'ellipsis',
          }
        },
        { selector: 'node[node_type = "Entry"]', style: { 'background-color': '#1f6feb', 'border-color': '#58a6ff', 'color': '#f0f6fc' } },
        { selector: 'node[node_type = "Return"]', style: { 'background-color': '#238636', 'border-color': '#3fb950', 'color': '#f0f6fc', 'width': '90px' } },
        { selector: 'node[node_type = "Revert"]', style: { 'background-color': '#da3633', 'border-color': '#f85149', 'color': '#f0f6fc', 'width': '90px' } },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        { selector: 'edge', style: { 'width': 1, 'line-color': '#30363d', 'target-arrow-color': '#30363d', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.7 } },
        { selector: 'edge[kind ^= "ConditionalTrue"]', style: { 'line-color': '#3fb95088', 'target-arrow-color': '#3fb950', 'label': '✓', 'font-size': '11px', 'color': '#3fb950' } },
        { selector: 'edge[kind ^= "ConditionalFalse"]', style: { 'line-color': '#f8514988', 'target-arrow-color': '#f85149', 'label': '✗', 'font-size': '11px', 'color': '#f85149' } },
      ],
      layout: { name: 'preset' },
      minZoom: 0.3, maxZoom: 3, wheelSensitivity: 0.3,
    });

    const cfgLayout = cfgInstance.layout({ name: 'dagre', rankDir: 'TB', nodeSep: 35, rankSep: 45, animate: false } as any);
    cfgLayout.run(); cfgLayout.stop();
  }

  function termColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : '#8b949e';
  }
</script>

<div class="contract-view">
  <div class="topbar">
    <a href="/">← Contracts</a>
    <span class="kind">{contract?.kind.toLowerCase() ?? ''}</span>
    <span class="name">{contract?.name ?? 'Loading...'}</span>
    {#if contract?.inherits.length}
      <span class="inherits">inherits {contract.inherits.join(', ')}</span>
    {/if}
    <div class="toolbar">
      <button class="tool-btn" onclick={toggleSearch}>🔍</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else}
    <div class="canvas-area">
      <!-- Call graph canvas -->
      <div class="cy-main" bind:this={cyContainer}></div>

      <!-- Inline CFG canvas (appears when function CFG is opened) -->
      {#if expandedCfg}
        <div class="cfg-pane">
          <div class="cfg-header">
            <span>CFG: {expandedCfg}</span>
            <button onclick={() => { expandedCfg = null; if (cfgInstance) { cfgInstance.destroy(); cfgInstance = null; } }}>✕</button>
          </div>
          <div class="cy-cfg" bind:this={cfgContainer}></div>
          <a href="/contract/{contract?.name}/{expandedCfg}" class="full-link">Open full view →</a>
        </div>
      {/if}
    </div>

    <!-- Function detail panel -->
    {#if selectedFunc && contract}
      <DraggablePanel
        title={selectedFunc.label || selectedFunc.name}
        x={window.innerWidth - 380} y={60} width={360}
        onclose={() => { selectedFunc = null; cyInstance?.elements().removeClass('selected'); }}
      >
        <div class="func-detail">
          <div class="fd-row">
            <span class="fd-label">Visibility</span>
            <span>{selectedFunc.visibility?.toLowerCase()}</span>
          </div>
          <div class="fd-row">
            <span class="fd-label">Mutability</span>
            <span>{selectedFunc.mutability?.toLowerCase()}</span>
          </div>
          {#if selectedFunc.params?.length > 0}
            <div class="fd-row">
              <span class="fd-label">Params</span>
              <span class="fd-mono">{selectedFunc.params.map((p: any) => `${p.type_name} ${p.name}`).join(', ')}</span>
            </div>
          {/if}
          <div class="fd-row">
            <span class="fd-label">Paths</span>
            <span>
              {selectedFunc.path_count} total
              <span class="g">{selectedFunc.happy_paths}✓</span>
              <span class="r">{selectedFunc.revert_paths}✗</span>
            </span>
          </div>

          <button class="show-cfg-btn" onclick={() => showCfg(selectedFunc.label || selectedFunc.name)}>
            {expandedCfg === (selectedFunc.label || selectedFunc.name) ? 'Hide CFG' : 'Show CFG →'}
          </button>

          {#if funcPaths[selectedFunc.label || selectedFunc.name]}
            <div class="fd-paths-title">Paths</div>
            {#each funcPaths[selectedFunc.label || selectedFunc.name].paths as path}
              <a href="/contract/{contract.name}/{selectedFunc.label || selectedFunc.name}?path={path.id}" class="fd-path">
                <span class="pid">#{path.id}</span>
                <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
                <span class="pdepth">{path.nodes.length}blk</span>
                {#if path.annotations.external_calls.length > 0}
                  <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
                {/if}
                {#if path.annotations.state_writes.length > 0}
                  <span class="pb wr">✏{path.annotations.state_writes.length}</span>
                {/if}
              </a>
            {/each}
          {/if}
        </div>
      </DraggablePanel>
    {/if}

    <div class="legend">
      <span><span class="dot" style="background:#238636"></span>Internal</span>
      <span><span class="dot" style="background:#161b22;border:1px solid #f85149"></span>External</span>
      <span>Click function → details · Scroll zoom · Drag pan</span>
    </div>
  {/if}
</div>

<style>
  .contract-view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #0d1117; }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px; background: #161b22; border-bottom: 1px solid #30363d;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .kind { font-size: 12px; color: #8b949e; }
  .name { font-size: 16px; font-weight: 700; color: #f0f6fc; }
  .inherits { font-size: 11px; color: #484f58; font-style: italic; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .tool-btn {
    background: #21262d; border: 1px solid #30363d; color: #c9d1d9;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .tool-btn:hover { border-color: #58a6ff; }

  .error { padding: 24px; color: #f85149; }

  .canvas-area { flex: 1; display: flex; overflow: hidden; }
  .cy-main { flex: 1; }
  .cfg-pane {
    width: 400px; flex-shrink: 0;
    border-left: 1px solid #30363d;
    display: flex; flex-direction: column;
    background: #0d1117;
  }
  .cfg-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 8px 12px; border-bottom: 1px solid #21262d;
    font-size: 13px; color: #58a6ff; font-weight: 600;
  }
  .cfg-header button {
    background: none; border: none; color: #8b949e; cursor: pointer; font-size: 14px;
  }
  .cy-cfg { flex: 1; }
  .full-link {
    display: block; text-align: center; padding: 8px;
    font-size: 11px; color: #58a6ff; border-top: 1px solid #21262d;
  }

  .func-detail { padding: 8px; }
  .fd-row { display: flex; justify-content: space-between; padding: 4px 0; font-size: 12px; }
  .fd-label { color: #8b949e; }
  .fd-mono { font-family: monospace; font-size: 11px; color: #c9d1d9; }
  .g { color: #3fb950; }
  .r { color: #f85149; }

  .show-cfg-btn {
    width: 100%; margin: 8px 0;
    background: #21262d; border: 1px solid #30363d;
    color: #58a6ff; padding: 6px; border-radius: 4px;
    cursor: pointer; font-size: 12px;
  }
  .show-cfg-btn:hover { border-color: #58a6ff; }

  .fd-paths-title { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; }

  .fd-path {
    display: flex; align-items: center; gap: 4px;
    padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit;
  }
  .fd-path:hover { background: #0d1117; text-decoration: none; }
  .pid { color: #484f58; font-weight: 600; }
  .pdepth { color: #484f58; font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: #f851491a; color: #f85149; }
  .pb.wr { background: #58a6ff1a; color: #58a6ff; }

  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px; font-size: 11px; color: #8b949e;
    background: #161b22cc; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #30363d; z-index: 10;
  }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }
</style>
