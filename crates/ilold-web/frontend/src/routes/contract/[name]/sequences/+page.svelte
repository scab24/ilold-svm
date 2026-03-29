<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { getSequences, getContract, type ContractDetail } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';

  let contract: ContractDetail | null = $state(null);
  let seqTree: any = $state(null);
  let error: string | null = $state(null);

  // Filters
  let filterDepth: number | null = $state(null);
  let filterFunc: string | null = $state(null);
  let sortBy: string = $state('paths');
  let onlyStateChange: boolean = $state(false);
  let selectedSeq: any = $state(null);

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

  const filteredSequences = $derived(() => {
    if (!seqTree) return [];
    let seqs = [...seqTree.sequences];

    if (filterDepth !== null) {
      seqs = seqs.filter((s: any) => s.depth === filterDepth);
    }
    if (filterFunc !== null) {
      seqs = seqs.filter((s: any) => s.steps.some((i: number) => seqTree.functions[i].name === filterFunc));
    }
    if (onlyStateChange) {
      seqs = seqs.filter((s: any) => s.has_state_change);
    }

    if (sortBy === 'paths') {
      seqs.sort((a: any, b: any) => b.path_count - a.path_count);
    } else if (sortBy === 'depth') {
      seqs.sort((a: any, b: any) => b.depth - a.depth);
    }

    return seqs;
  });

  // Build 8x8 matrix
  const matrix = $derived(() => {
    if (!seqTree) return [];
    const funcs = seqTree.functions;
    const n = funcs.length;
    const grid: {from: string, to: string, count: number, totalPaths: number, hasStateChange: boolean}[][] = [];

    for (let i = 0; i < n; i++) {
      grid[i] = [];
      for (let j = 0; j < n; j++) {
        const matching = seqTree.sequences.filter((s: any) =>
          s.depth >= 2 && s.steps[0] === i && s.steps[1] === j
        );
        grid[i][j] = {
          from: funcs[i].name,
          to: funcs[j].name,
          count: matching.length,
          totalPaths: matching.reduce((sum: number, s: any) => sum + Number(s.path_count), 0),
          hasStateChange: matching.some((s: any) => s.has_state_change),
        };
      }
    }
    return grid;
  });

  function seqSteps(seq: any): string {
    return seq.steps.map((i: number) => seqTree.functions[i].name).join(' → ');
  }

  function cellColor(totalPaths: number, maxPaths: number): string {
    if (totalPaths === 0) return 'transparent';
    const intensity = Math.min(totalPaths / Math.max(maxPaths, 1), 1);
    const r = Math.round(30 + intensity * 180);
    const g = Math.round(30 + intensity * 40);
    const b = Math.round(50);
    return `rgb(${r},${g},${b})`;
  }

  function maxMatrixPaths(): number {
    const m = matrix();
    let max = 0;
    for (const row of m) {
      for (const cell of row) {
        if (cell.totalPaths > max) max = cell.totalPaths;
      }
    }
    return max;
  }
</script>

<div class="seq-view">
  <div class="topbar">
    <a href="/contract/{contractName}">← {contractName}</a>
    <span class="title">Function Sequences</span>
    {#if seqTree}
      <span class="stats">{seqTree.sequences.length} sequences · depth {seqTree.max_depth} · {seqTree.functions.length} functions</span>
    {/if}
    <div class="toolbar">
      <button class="tbtn" onclick={toggleSearch}>🔍</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else if !seqTree}
    <div class="loading">Loading...</div>
  {:else}
    <div class="content">
      <!-- Transition Matrix -->
      <div class="section">
        <h2>Transition Matrix</h2>
        <p class="desc">Each cell shows the combined paths when function (row) is followed by function (column). Darker = more paths.</p>
        <div class="matrix-wrapper">
          <table class="matrix">
            <thead>
              <tr>
                <th></th>
                {#each seqTree.functions as f}
                  <th class="matrix-header">{f.name}</th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each matrix() as row, i}
                <tr>
                  <td class="matrix-row-label">{seqTree.functions[i].name}</td>
                  {#each row as cell}
                    <td
                      class="matrix-cell"
                      style="background:{cellColor(cell.totalPaths, maxMatrixPaths())}"
                      title="{cell.from} → {cell.to}: {cell.totalPaths} combined paths"
                      onclick={() => { filterFunc = null; filterDepth = 2; selectedSeq = null; }}
                    >
                      {#if cell.totalPaths > 0}
                        {cell.totalPaths}
                      {/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <!-- Filters -->
      <div class="filters">
        <div class="filter-group">
          <label>Depth:</label>
          <button class:active={filterDepth === null} onclick={() => filterDepth = null}>All</button>
          {#each Array.from({length: seqTree.max_depth}, (_, i) => i + 1) as d}
            <button class:active={filterDepth === d} onclick={() => filterDepth = d}>{d}</button>
          {/each}
        </div>
        <div class="filter-group">
          <label>Contains:</label>
          <button class:active={filterFunc === null} onclick={() => filterFunc = null}>Any</button>
          {#each seqTree.functions as f}
            <button class:active={filterFunc === f.name} onclick={() => filterFunc = f.name}>{f.name}</button>
          {/each}
        </div>
        <div class="filter-group">
          <label>Sort:</label>
          <button class:active={sortBy === 'paths'} onclick={() => sortBy = 'paths'}>Paths ↓</button>
          <button class:active={sortBy === 'depth'} onclick={() => sortBy = 'depth'}>Depth ↓</button>
        </div>
        <div class="filter-group">
          <label>
            <input type="checkbox" bind:checked={onlyStateChange} /> State-changing only
          </label>
        </div>
        <div class="filter-result">{filteredSequences().length} sequences</div>
      </div>

      <!-- Sequence List -->
      <div class="seq-list">
        {#each filteredSequences().slice(0, 200) as seq}
          <button
            class="seq-row"
            class:selected={selectedSeq === seq}
            onclick={() => selectedSeq = selectedSeq === seq ? null : seq}
          >
            <span class="seq-depth">D{seq.depth}</span>
            <span class="seq-steps">{seqSteps(seq)}</span>
            <span class="seq-paths">{seq.path_count}p</span>
            {#if seq.has_state_change}
              <span class="seq-badge sc">state</span>
            {:else}
              <span class="seq-badge ro">read</span>
            {/if}
          </button>
        {/each}
        {#if filteredSequences().length > 200}
          <div class="more">Showing 200 of {filteredSequences().length}. Use filters to narrow down.</div>
        {/if}
      </div>

      <!-- Selected sequence detail -->
      {#if selectedSeq}
        <div class="seq-detail">
          <h3>{seqSteps(selectedSeq)}</h3>
          <div class="detail-row">
            <span class="dl">Depth</span>
            <span>{selectedSeq.depth}</span>
          </div>
          <div class="detail-row">
            <span class="dl">Combined paths</span>
            <span>{selectedSeq.path_count}</span>
          </div>
          <div class="detail-row">
            <span class="dl">State change</span>
            <span style="color:{selectedSeq.has_state_change ? '#d29922' : '#3fb950'}">{selectedSeq.has_state_change ? 'Yes' : 'No (read-only)'}</span>
          </div>
          <div class="detail-steps">
            {#each selectedSeq.steps as stepIdx, i}
              {@const func = seqTree.functions[stepIdx]}
              <div class="step">
                <span class="step-num">{i + 1}</span>
                <a href="/contract/{contractName}/{func.name}" class="step-name">{func.name}</a>
                <span class="step-info">{func.path_count}p · {func.read_only ? 'view' : 'state-changing'}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .seq-view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #0d1117; }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px; background: #161b22; border-bottom: 1px solid #30363d;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .title { font-size: 16px; font-weight: 700; color: #f0f6fc; }
  .stats { font-size: 12px; color: #8b949e; }
  .toolbar { margin-left: auto; }
  .tbtn { background: #21262d; border: 1px solid #30363d; color: #c9d1d9; padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; }

  .error { padding: 24px; color: #f85149; }
  .loading { padding: 24px; color: #8b949e; }

  .content { flex: 1; overflow-y: auto; padding: 20px 24px; max-width: 1200px; margin: 0 auto; width: 100%; box-sizing: border-box; }

  .section { margin-bottom: 24px; }
  .section h2 { font-size: 16px; color: #f0f6fc; margin: 0 0 4px; }
  .desc { font-size: 12px; color: #8b949e; margin: 0 0 12px; }

  .matrix-wrapper { overflow-x: auto; }
  .matrix { border-collapse: collapse; font-size: 11px; }
  .matrix th, .matrix td { padding: 4px 6px; text-align: center; }
  .matrix-header { color: #8b949e; font-weight: 600; writing-mode: vertical-rl; transform: rotate(180deg); max-width: 30px; font-size: 10px; }
  .matrix-row-label { color: #8b949e; font-weight: 600; text-align: right; padding-right: 8px; font-size: 10px; white-space: nowrap; }
  .matrix-cell {
    min-width: 40px; height: 32px;
    border: 1px solid #21262d; border-radius: 2px;
    color: #c9d1d9; font-size: 10px; font-family: monospace;
    cursor: pointer;
  }
  .matrix-cell:hover { border-color: #58a6ff; }

  .filters {
    display: flex; flex-wrap: wrap; gap: 12px; align-items: center;
    padding: 12px 0; border-top: 1px solid #21262d; border-bottom: 1px solid #21262d;
    margin-bottom: 12px;
  }
  .filter-group { display: flex; align-items: center; gap: 4px; font-size: 11px; color: #8b949e; }
  .filter-group label { font-size: 11px; color: #8b949e; display: flex; align-items: center; gap: 4px; }
  .filter-group button {
    background: #21262d; border: 1px solid #30363d; color: #8b949e;
    padding: 2px 8px; border-radius: 4px; cursor: pointer; font-size: 10px;
  }
  .filter-group button:hover { border-color: #58a6ff; color: #c9d1d9; }
  .filter-group button.active { background: #1f6feb; border-color: #58a6ff; color: #f0f6fc; }
  .filter-result { font-size: 12px; color: #58a6ff; margin-left: auto; }

  .seq-list { display: flex; flex-direction: column; gap: 2px; }
  .seq-row {
    display: flex; align-items: center; gap: 8px;
    padding: 6px 10px; background: #161b22; border: 1px solid #21262d;
    border-radius: 4px; cursor: pointer; font: inherit; font-size: 12px;
    color: inherit; text-align: left; width: 100%;
  }
  .seq-row:hover { border-color: #30363d; }
  .seq-row.selected { border-color: #58a6ff; background: #161b22ee; }
  .seq-depth { color: #484f58; font-size: 10px; font-weight: 600; min-width: 24px; }
  .seq-steps { flex: 1; font-family: monospace; color: #c9d1d9; }
  .seq-paths { color: #8b949e; font-size: 11px; }
  .seq-badge { font-size: 9px; padding: 1px 6px; border-radius: 8px; }
  .seq-badge.sc { background: #d299221a; color: #d29922; }
  .seq-badge.ro { background: #3fb9501a; color: #3fb950; }
  .more { text-align: center; padding: 12px; font-size: 11px; color: #484f58; }

  .seq-detail {
    margin-top: 16px; padding: 14px;
    background: #161b22; border: 1px solid #30363d; border-radius: 8px;
  }
  .seq-detail h3 { font-size: 14px; font-family: monospace; color: #f0f6fc; margin: 0 0 10px; }
  .detail-row { display: flex; justify-content: space-between; font-size: 12px; padding: 3px 0; }
  .dl { color: #8b949e; }
  .detail-steps { margin-top: 10px; display: flex; flex-direction: column; gap: 4px; }
  .step {
    display: flex; align-items: center; gap: 8px;
    padding: 6px 10px; background: #0d1117; border-radius: 4px;
  }
  .step-num { font-size: 10px; color: #484f58; font-weight: 700; min-width: 18px; }
  .step-name { font-family: monospace; font-weight: 600; color: #58a6ff; }
  .step-info { font-size: 11px; color: #8b949e; margin-left: auto; }
</style>
