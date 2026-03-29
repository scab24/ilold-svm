<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { getContract, getPaths, getSequences, type ContractDetail } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';

  let contract: ContractDetail | null = $state(null);
  let error: string | null = $state(null);
  let funcPaths: Record<string, any> = $state({});
  let expandedFuncs: Set<string> = $state(new Set());
  let sequenceInfo: any = $state(null);

  // Position state for each function card
  let positions: Record<string, {x: number, y: number}> = $state({});
  let dragging: string | null = null;
  let dragOffset = {x: 0, y: 0};

  // Canvas pan state
  let canvasOffset = $state({x: 0, y: 0});
  let panning = false;
  let panStart = {x: 0, y: 0};

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    setSearchContext(contractName);
    try {
      contract = await getContract(contractName);
      try { sequenceInfo = await getSequences(contractName); } catch {}

      // Auto-layout: grid positions
      if (contract) {
        const cols = Math.ceil(Math.sqrt(contract.functions.length));
        contract.functions.forEach((f, i) => {
          const col = i % cols;
          const row = Math.floor(i / cols);
          positions[f.name || 'constructor'] = { x: 40 + col * 320, y: 40 + row * 200 };
        });
        positions = { ...positions };
      }
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  });

  onDestroy(() => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  });

  function onCardMouseDown(name: string, e: MouseEvent) {
    if ((e.target as HTMLElement).tagName === 'A' ||
        (e.target as HTMLElement).tagName === 'BUTTON' ||
        (e.target as HTMLElement).closest('.func-expanded')) return;
    dragging = name;
    const pos = positions[name] || {x: 0, y: 0};
    dragOffset = { x: e.clientX - pos.x - canvasOffset.x, y: e.clientY - pos.y - canvasOffset.y };
    e.preventDefault();
  }

  function onCanvasMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      panning = true;
      panStart = { x: e.clientX - canvasOffset.x, y: e.clientY - canvasOffset.y };
    }
  }

  function onMouseMove(e: MouseEvent) {
    if (dragging) {
      positions[dragging] = {
        x: e.clientX - dragOffset.x - canvasOffset.x,
        y: e.clientY - dragOffset.y - canvasOffset.y,
      };
      positions = { ...positions };
    } else if (panning) {
      canvasOffset = {
        x: e.clientX - panStart.x,
        y: e.clientY - panStart.y,
      };
    }
  }

  function onMouseUp() {
    dragging = null;
    panning = false;
  }

  async function toggleExpand(funcName: string) {
    if (expandedFuncs.has(funcName)) {
      expandedFuncs.delete(funcName);
      expandedFuncs = new Set(expandedFuncs);
    } else {
      expandedFuncs.add(funcName);
      expandedFuncs = new Set(expandedFuncs);
      if (!funcPaths[funcName] && contract) {
        try {
          funcPaths[funcName] = await getPaths(contract.name, funcName);
          funcPaths = { ...funcPaths };
        } catch {}
      }
    }
  }

  function mutColor(m: string): string {
    if (m === 'View' || m === 'Pure') return '#1f6feb';
    return '#238636';
  }

  function termColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : '#8b949e';
  }
</script>

<div class="contract-view">
  <div class="topbar">
    <a href="/">← Contracts</a>
    <span class="contract-kind">{contract?.kind.toLowerCase() ?? ''}</span>
    <span class="contract-name">{contract?.name ?? 'Loading...'}</span>
    {#if contract?.inherits.length}
      <span class="inherits">inherits {contract.inherits.join(', ')}</span>
    {/if}
    {#if sequenceInfo}
      <span class="seq-badge">{sequenceInfo.sequences.length} sequences</span>
    {/if}
    <div class="toolbar">
      <button class="tool-btn" onclick={toggleSearch}>🔍</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else if !contract}
    <div class="loading">Loading...</div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="canvas" onmousedown={onCanvasMouseDown} style="cursor:{panning ? 'grabbing' : 'default'}">
      <div class="canvas-inner" style="transform: translate({canvasOffset.x}px, {canvasOffset.y}px)">

        {#each contract.functions as func}
          {@const name = func.name || 'constructor'}
          {@const pos = positions[name] || {x: 0, y: 0}}
          {@const isExpanded = expandedFuncs.has(name)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="func-card"
            class:expanded={isExpanded}
            style="left:{pos.x}px; top:{pos.y}px"
            onmousedown={(e) => onCardMouseDown(name, e)}
          >
            <div class="card-header" style="border-left: 3px solid {mutColor(func.mutability)}">
              <div class="card-top">
                <span class="card-name">{name}</span>
                <span class="card-vis">{func.visibility.toLowerCase()}</span>
              </div>
              <div class="card-stats">
                <span>{func.path_count} paths</span>
                {#if func.happy_paths > 0}<span class="g">{func.happy_paths}✓</span>{/if}
                {#if func.revert_paths > 0}<span class="r">{func.revert_paths}✗</span>{/if}
              </div>
              {#if func.params.length > 0}
                <div class="card-params">({func.params.map(p => p.type_name).join(', ')})</div>
              {/if}
              <button class="expand-btn" onclick={() => toggleExpand(name)}>
                {isExpanded ? '▲ Collapse' : '▼ Expand paths'}
              </button>
            </div>

            {#if isExpanded}
              <div class="func-expanded">
                <a href="/contract/{contract.name}/{name}" class="view-cfg">View CFG →</a>

                {#if funcPaths[name]}
                  {#each funcPaths[name].paths as path}
                    <a href="/contract/{contract.name}/{name}?path={path.id}" class="path-row">
                      <span class="pid">#{path.id}</span>
                      <span style="color:{termColor(path.terminal)};font-weight:600;font-size:11px">{path.terminal}</span>
                      <span class="pdepth">{path.nodes.length}blk</span>
                      {#if path.annotations.external_calls.length > 0}
                        <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
                      {/if}
                      {#if path.annotations.state_writes.length > 0}
                        <span class="pb wr">✏{path.annotations.state_writes.length}</span>
                      {/if}
                    </a>
                  {/each}
                {:else}
                  <div class="loading-sm">Loading...</div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}

        <!-- State variables card -->
        {#if contract.state_vars.length > 0}
          <div class="vars-card" style="left:{40}px; top:{40 + Math.ceil(contract.functions.length / Math.ceil(Math.sqrt(contract.functions.length))) * 200 + 20}px">
            <div class="vars-title">State Variables ({contract.state_vars.length})</div>
            {#each contract.state_vars as sv}
              <div class="var-row">
                <span class="vname">{sv.name}</span>
                <span class="vtype">{sv.type_name}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="legend">
      <span><span class="dot" style="background:#238636"></span>State-changing</span>
      <span><span class="dot" style="background:#1f6feb"></span>View/Pure</span>
      <span>Drag cards · Pan canvas · Click expand</span>
    </div>
  {/if}
</div>

<style>
  .contract-view {
    position: fixed; inset: 0;
    display: flex; flex-direction: column;
    background: #0d1117;
  }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px;
    background: #161b22; border-bottom: 1px solid #30363d;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .contract-kind { font-size: 12px; color: #8b949e; }
  .contract-name { font-size: 16px; font-weight: 700; color: #f0f6fc; }
  .inherits { font-size: 11px; color: #484f58; font-style: italic; }
  .seq-badge { font-size: 10px; background: #21262d; border: 1px solid #30363d; padding: 2px 8px; border-radius: 10px; color: #8b949e; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .tool-btn {
    background: #21262d; border: 1px solid #30363d; color: #c9d1d9;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .tool-btn:hover { border-color: #58a6ff; }

  .error { padding: 24px; color: #f85149; }
  .loading { padding: 24px; color: #8b949e; }

  .canvas {
    flex: 1; overflow: hidden; position: relative;
  }
  .canvas-inner {
    position: absolute; top: 0; left: 0;
    width: 10000px; height: 10000px;
  }

  .func-card {
    position: absolute;
    width: 280px;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 10px;
    cursor: grab;
    user-select: none;
    transition: box-shadow 0.15s;
    z-index: 1;
  }
  .func-card:hover { box-shadow: 0 4px 16px #00000044; }
  .func-card.expanded { z-index: 2; }
  .func-card:active { cursor: grabbing; }

  .card-header { padding: 10px 12px; }
  .card-top { display: flex; align-items: center; gap: 6px; }
  .card-name { font-weight: 700; font-family: monospace; font-size: 14px; color: #f0f6fc; flex: 1; }
  .card-vis { font-size: 10px; color: #484f58; }
  .card-stats { font-size: 11px; color: #8b949e; display: flex; gap: 4px; margin-top: 2px; }
  .card-stats .g { color: #3fb950; }
  .card-stats .r { color: #f85149; }
  .card-params { font-size: 10px; color: #484f58; font-family: monospace; margin-top: 2px; }

  .expand-btn {
    margin-top: 6px;
    background: #21262d; border: 1px solid #30363d;
    color: #8b949e; padding: 3px 10px;
    border-radius: 4px; cursor: pointer; font-size: 10px;
    width: 100%;
  }
  .expand-btn:hover { border-color: #58a6ff; color: #c9d1d9; }

  .func-expanded {
    padding: 8px 12px;
    border-top: 1px solid #21262d;
    max-height: 300px;
    overflow-y: auto;
    cursor: default;
  }

  .view-cfg {
    display: inline-block; padding: 4px 10px;
    background: #21262d; border: 1px solid #30363d;
    border-radius: 4px; font-size: 11px; color: #58a6ff;
    margin-bottom: 6px;
  }
  .view-cfg:hover { border-color: #58a6ff; text-decoration: none; }

  .path-row {
    display: flex; align-items: center; gap: 4px;
    padding: 3px 4px; border-radius: 3px;
    font-size: 11px; color: inherit;
  }
  .path-row:hover { background: #0d1117; text-decoration: none; }
  .pid { color: #484f58; font-weight: 600; min-width: 20px; }
  .pdepth { color: #484f58; font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: #f851491a; color: #f85149; }
  .pb.wr { background: #58a6ff1a; color: #58a6ff; }

  .loading-sm { font-size: 11px; color: #484f58; }

  .vars-card {
    position: absolute;
    width: 300px;
    background: #161b22;
    border: 1px solid #21262d;
    border-radius: 8px;
    padding: 8px 12px;
    z-index: 0;
  }
  .vars-title { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 4px; font-weight: 600; }
  .var-row { display: flex; justify-content: space-between; padding: 2px 0; font-family: monospace; font-size: 11px; }
  .vname { color: #c9d1d9; }
  .vtype { color: #484f58; font-size: 10px; max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

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
</style>
