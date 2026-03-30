<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getSequences, getContract, type ContractDetail } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';

  let contract: ContractDetail | null = $state(null);
  let seqTree: any = $state(null);
  let error: string | null = $state(null);

  let filterDepth: number | null = $state(null);
  let filterFunc: string | null = $state(null);
  let sortBy: string = $state('paths');
  let onlyStateChange: boolean = $state(false);
  let selectedSeq: any = $state(null);
  let viewMode: string = $state('tree'); // 'tree' or 'list'

  let cyContainer: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;

  const contractName = $derived(page.params.name);

  onMount(async () => {
    if (!contractName) return;
    setSearchContext(contractName);
    try {
      contract = await getContract(contractName);
      seqTree = await getSequences(contractName);
    } catch (e) {
      error = `Failed to load sequences for ${contractName}`;
    }
  });

  onDestroy(() => {
    if (cyInstance) { cyInstance.destroy(); cyInstance = null; }
  });

  // Computed filtered sequences
  let filtered: any[] = $state([]);
  $effect(() => {
    if (!seqTree) { filtered = []; return; }
    let seqs = [...seqTree.sequences];
    if (filterDepth !== null) seqs = seqs.filter((s: any) => s.depth === filterDepth);
    if (filterFunc !== null) seqs = seqs.filter((s: any) => s.steps.some((i: number) => seqTree.functions[i].name === filterFunc));
    if (onlyStateChange) seqs = seqs.filter((s: any) => s.has_state_change);
    if (sortBy === 'paths') seqs.sort((a: any, b: any) => b.path_count - a.path_count);
    else if (sortBy === 'depth') seqs.sort((a: any, b: any) => b.depth - a.depth);
    filtered = seqs;
  });

  // Render tree when view mode changes or data loads
  $effect(() => {
    if (viewMode === 'tree' && seqTree && cyContainer) {
      setTimeout(() => renderTree(), 100);
    }
  });

  async function renderTree() {
    if (!seqTree || !cyContainer) return;

    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    // Build tree from sequences: shared prefix structure
    const tree: Record<string, {children: Record<string, any>, pathCount: number, hasStateChange: boolean}> = {};

    for (const seq of seqTree.sequences) {
      let current = tree;
      for (let i = 0; i < seq.steps.length; i++) {
        const funcName = seqTree.functions[seq.steps[i]].name;
        const key = funcName;
        if (!current[key]) {
          current[key] = { children: {}, pathCount: 0, hasStateChange: false };
        }
        current[key].pathCount += Number(seq.path_count);
        if (seq.has_state_change) current[key].hasStateChange = true;
        current = current[key].children;
      }
    }

    // Convert tree to Cytoscape elements
    const elements: any[] = [];
    let nodeId = 0;

    // Root node
    elements.push({
      group: 'nodes',
      data: { id: 'root', label: contract?.name || 'Contract', _type: 'root' },
      classes: 'root',
    });

    function addNodes(subtree: Record<string, any>, parentId: string, depth: number) {
      for (const [funcName, data] of Object.entries(subtree)) {
        const id = `n${nodeId++}`;
        const readOnly = seqTree.functions.find((f: any) => f.name === funcName)?.read_only ?? false;
        elements.push({
          group: 'nodes',
          data: {
            id,
            label: funcName,
            pathCount: data.pathCount,
            hasStateChange: data.hasStateChange,
            readOnly,
            depth,
            _type: 'seq-func',
          },
          classes: `seq-func ${readOnly ? 'readonly' : ''} ${data.hasStateChange ? 'state-change' : ''}`,
        });
        elements.push({
          group: 'edges',
          data: { source: parentId, target: id },
        });
        if (Object.keys(data.children).length > 0 && depth < 2) {
          addNodes(data.children, id, depth + 1);
        }
      }
    }

    addNodes(tree, 'root', 0);

    cyInstance = cytoscape({
      container: cyContainer,
      elements,
      style: [
        {
          selector: 'node.root',
          style: {
            'background-color': '#3a6b9f', 'label': 'data(label)', 'color': '#b8c4d4',
            'font-size': '13px', 'font-weight': 'bold',
            'text-valign': 'center', 'text-halign': 'center',
            'width': '140px', 'height': '40px', 'shape': 'roundrectangle',
          }
        },
        {
          selector: 'node.seq-func',
          style: {
            'background-color': '#5a9a6a', 'label': 'data(label)', 'color': '#b8c4d4',
            'font-size': '10px', 'text-valign': 'center', 'text-halign': 'center',
            'width': '110px', 'height': '30px', 'shape': 'roundrectangle',
          }
        },
        {
          selector: 'node.seq-func.readonly',
          style: { 'background-color': '#3a6b9f' }
        },
        {
          selector: 'node.seq-func.state-change',
          style: { 'background-color': '#5a9a6a' }
        },
        { selector: 'node:active', style: { 'overlay-opacity': 0 } },
        {
          selector: 'edge',
          style: {
            'width': 1, 'line-color': '#2a2d38', 'target-arrow-color': '#4a5568',
            'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.6,
          }
        },
      ] as any,
      layout: { name: 'preset' },
      minZoom: 0.05, maxZoom: 4, wheelSensitivity: 0.3,
    });

    const layout = cyInstance.layout({
      name: 'dagre',
      rankDir: 'LR',
      nodeSep: 15,
      rankSep: 60,
      animate: false,
    } as any);
    layout.run();
    layout.stop();

    cyInstance.on('tap', 'node.seq-func', (evt: any) => {
      const funcName = evt.target.data('label');
      if (funcName && contractName) {
        window.location.href = `/contract/${contractName}/${funcName}`;
      }
    });

    cyInstance.on('mouseover', 'node.seq-func', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
  }

  function seqSteps(seq: any): string {
    return seq.steps.map((i: number) => seqTree.functions[i].name).join(' → ');
  }
</script>

<div class="seq-view">
  <div class="topbar">
    <a href="/contract/{contractName}">← {contractName}</a>
    <span class="title">Function Sequences</span>
    {#if seqTree}
      <span class="stats">{seqTree.sequences.length} sequences · depth {seqTree.max_depth}</span>
    {/if}
    <div class="toolbar">
      <button class="tbtn" class:active={viewMode === 'tree'} onclick={() => viewMode = 'tree'}>🌳 Tree</button>
      <button class="tbtn" class:active={viewMode === 'list'} onclick={() => viewMode = 'list'}>📋 List</button>
      <button class="tbtn" onclick={toggleSearch}>🔍</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else if !seqTree}
    <div class="loading">Loading...</div>
  {:else}
    {#if viewMode === 'tree'}
      <div class="tree-canvas" bind:this={cyContainer}></div>
      <div class="legend">
        <span><span class="dot" style="background:#3a6b9f"></span>Contract / Read-only</span>
        <span><span class="dot" style="background:#5a9a6a"></span>State-changing</span>
        <span>Left→Right = sequence depth · Click function → view paths</span>
      </div>
    {:else}
      <div class="list-content">
        <!-- Filters -->
        <div class="filters">
          <div class="fg">
            <span class="fl">Depth:</span>
            <button class:active={filterDepth === null} onclick={() => filterDepth = null}>All</button>
            {#each Array.from({length: seqTree.max_depth}, (_, i) => i + 1) as d}
              <button class:active={filterDepth === d} onclick={() => filterDepth = d}>{d}</button>
            {/each}
          </div>
          <div class="fg">
            <span class="fl">Contains:</span>
            <button class:active={filterFunc === null} onclick={() => filterFunc = null}>Any</button>
            {#each seqTree.functions as f}
              <button class:active={filterFunc === f.name} onclick={() => filterFunc = f.name}>{f.name}</button>
            {/each}
          </div>
          <div class="fg">
            <span class="fl">Sort:</span>
            <button class:active={sortBy === 'paths'} onclick={() => sortBy = 'paths'}>Paths ↓</button>
            <button class:active={sortBy === 'depth'} onclick={() => sortBy = 'depth'}>Depth ↓</button>
          </div>
          <div class="fg">
            <label><input type="checkbox" bind:checked={onlyStateChange} /> State-changing only</label>
          </div>
          <span class="filter-count">{filtered.length} sequences</span>
        </div>

        <div class="seq-list">
          {#each filtered.slice(0, 200) as seq}
            <button class="seq-row" class:selected={selectedSeq === seq} onclick={() => selectedSeq = selectedSeq === seq ? null : seq}>
              <span class="sd">D{seq.depth}</span>
              <span class="ss">{seqSteps(seq)}</span>
              <span class="sp">{seq.path_count}p</span>
              {#if seq.has_state_change}
                <span class="sb sc">state</span>
              {:else}
                <span class="sb ro">read</span>
              {/if}
            </button>
          {/each}
          {#if filtered.length > 200}
            <div class="more">Showing 200 of {filtered.length}</div>
          {/if}
        </div>

        {#if selectedSeq}
          <div class="seq-detail">
            <h3>{seqSteps(selectedSeq)}</h3>
            <div class="dr"><span class="dl">Combined paths</span><span>{selectedSeq.path_count}</span></div>
            <div class="dr"><span class="dl">State change</span><span style="color:{selectedSeq.has_state_change ? '#c49a4a' : '#5a9a6a'}">{selectedSeq.has_state_change ? 'Yes' : 'No'}</span></div>
            <div class="steps">
              {#each selectedSeq.steps as stepIdx, i}
                {@const func = seqTree.functions[stepIdx]}
                <div class="step">
                  <span class="sn">{i + 1}</span>
                  <a href="/contract/{contractName}/{func.name}">{func.name}</a>
                  <span class="si">{func.path_count}p · {func.read_only ? 'view' : 'state'}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .seq-view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #181a20; }
  .topbar { display: flex; align-items: center; gap: 10px; padding: 8px 16px; background: #1e2028; border-bottom: 1px solid #2a2d38; z-index: 10; flex-shrink: 0; }
  .topbar a { font-size: 13px; color: #6b7a8d; }
  .title { font-size: 16px; font-weight: 700; color: #b8c4d4; }
  .stats { font-size: 12px; color: #6b7a8d; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .tbtn { background: #252830; border: 1px solid #2a2d38; color: #b8c4d4; padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; }
  .tbtn:hover { border-color: #5b9bd5; }
  .tbtn.active { background: #3a6b9f; border-color: #5b9bd5; }
  .error { padding: 24px; color: #c07070; }
  .loading { padding: 24px; color: #6b7a8d; }

  /* Tree view */
  .tree-canvas { flex: 1; }
  .legend { position: fixed; bottom: 12px; left: 16px; display: flex; gap: 10px; font-size: 11px; color: #6b7a8d; background: #1e2028cc; padding: 6px 12px; border-radius: 6px; border: 1px solid #2a2d38; z-index: 10; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }

  /* List view */
  .list-content { flex: 1; overflow-y: auto; padding: 16px 24px; max-width: 1100px; margin: 0 auto; width: 100%; box-sizing: border-box; }
  .filters { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; padding: 10px 0; border-bottom: 1px solid #252830; margin-bottom: 10px; }
  .fg { display: flex; align-items: center; gap: 3px; }
  .fl { font-size: 10px; color: #6b7a8d; margin-right: 2px; }
  .fg button { background: #252830; border: 1px solid #2a2d38; color: #6b7a8d; padding: 2px 7px; border-radius: 4px; cursor: pointer; font-size: 10px; }
  .fg button:hover { border-color: #5b9bd5; color: #b8c4d4; }
  .fg button.active { background: #3a6b9f; border-color: #5b9bd5; color: #b8c4d4; }
  .fg label { font-size: 10px; color: #6b7a8d; display: flex; align-items: center; gap: 3px; }
  .filter-count { font-size: 11px; color: #5b9bd5; margin-left: auto; }

  .seq-list { display: flex; flex-direction: column; gap: 2px; }
  .seq-row { display: flex; align-items: center; gap: 6px; padding: 5px 8px; background: #1e2028; border: 1px solid #252830; border-radius: 4px; cursor: pointer; font: inherit; font-size: 11px; color: inherit; text-align: left; width: 100%; }
  .seq-row:hover { border-color: #2a2d38; }
  .seq-row.selected { border-color: #5b9bd5; }
  .sd { color: #4a5568; font-size: 10px; font-weight: 600; min-width: 22px; }
  .ss { flex: 1; font-family: monospace; color: #b8c4d4; }
  .sp { color: #6b7a8d; font-size: 10px; }
  .sb { font-size: 9px; padding: 1px 5px; border-radius: 6px; }
  .sb.sc { background: #c49a4a1a; color: #c49a4a; }
  .sb.ro { background: #5a9a6a1a; color: #5a9a6a; }
  .more { text-align: center; padding: 10px; font-size: 11px; color: #4a5568; }

  .seq-detail { margin-top: 12px; padding: 12px; background: #1e2028; border: 1px solid #2a2d38; border-radius: 8px; }
  .seq-detail h3 { font-size: 13px; font-family: monospace; color: #b8c4d4; margin: 0 0 8px; }
  .dr { display: flex; justify-content: space-between; font-size: 12px; padding: 2px 0; }
  .dl { color: #6b7a8d; }
  .steps { margin-top: 8px; display: flex; flex-direction: column; gap: 3px; }
  .step { display: flex; align-items: center; gap: 8px; padding: 5px 8px; background: #181a20; border-radius: 4px; }
  .sn { font-size: 10px; color: #4a5568; font-weight: 700; }
  .si { font-size: 10px; color: #6b7a8d; margin-left: auto; }
</style>
