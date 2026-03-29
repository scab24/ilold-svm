<script lang="ts">
  import { onMount } from 'svelte';
  import { getProjectMap, type ProjectMap, type MapContract } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';

  let projectMap: ProjectMap | null = $state(null);
  let error: string | null = $state(null);

  onMount(async () => {
    setSearchContext(null);
    try {
      projectMap = await getProjectMap();
    } catch (e) {
      error = 'Failed to connect. Is "ilold serve" running?';
    }
  });

  let contracts: any[] = $state([]);
  let interfaces: any[] = $state([]);

  $effect(() => {
    if (projectMap) {
      contracts = projectMap.contracts.filter(c => c.kind !== 'Interface');
      interfaces = projectMap.contracts.filter(c => c.kind === 'Interface');
    }
  });

  function mutColor(m: string): string {
    if (m === 'View' || m === 'Pure') return '#1f6feb';
    return '#3fb950';
  }
</script>

<div class="map-view">
  <div class="topbar">
    <span class="logo">ilold</span>
    <span class="subtitle">execution path analyzer</span>
    {#if projectMap}
      <span class="stats">{projectMap.contracts.length} contracts · {projectMap.relationships.length} cross-contract calls</span>
    {/if}
    <div class="toolbar">
      <button class="tool-btn" onclick={toggleSearch}>🔍 Search</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else if !projectMap}
    <div class="loading">Analyzing...</div>
  {:else}
    <div class="canvas-scroll">
      <div class="contract-grid">
        {#each contracts as contract}
          <div class="contract-card">
            <div class="card-header">
              <span class="card-kind">{contract.kind.toLowerCase()}</span>
              <h2><a href="/contract/{contract.name}">{contract.name}</a></h2>
              {#if contract.inherits.length > 0}
                <div class="card-inherits">inherits {contract.inherits.join(', ')}</div>
              {/if}
            </div>

            <div class="card-section">
              <div class="section-title">Functions</div>
              {#each contract.functions as func}
                <a href="/contract/{contract.name}/{func.name}" class="func-item">
                  <span class="func-dot" style="background:{mutColor(func.mutability)}"></span>
                  <span class="func-name">{func.name}</span>
                  <span class="func-vis">{func.visibility.toLowerCase()}</span>
                  {#if func.has_external_calls}
                    <span class="func-badge ext">ext</span>
                  {/if}
                  <span class="func-paths">
                    {func.path_count}p
                    {#if func.happy_paths > 0}<span class="g">{func.happy_paths}✓</span>{/if}
                    {#if func.revert_paths > 0}<span class="r">{func.revert_paths}✗</span>{/if}
                  </span>
                </a>
              {/each}
            </div>

            {#if contract.state_vars.length > 0}
              <div class="card-section">
                <div class="section-title">Variables</div>
                {#each contract.state_vars as sv}
                  <div class="var-item">
                    <span class="var-name">{sv.name}</span>
                    <span class="var-type">{sv.type_name}</span>
                  </div>
                {/each}
              </div>
            {/if}

            {#if projectMap.relationships.filter(r => r.from_contract === contract.name).length > 0}
              <div class="card-section">
                <div class="section-title">Calls to</div>
                {#each projectMap.relationships.filter(r => r.from_contract === contract.name) as rel}
                  <div class="rel-item">
                    <span class="rel-func">{rel.from_function}</span>
                    <span class="rel-arrow">→</span>
                    <span class="rel-target">{rel.to_contract}.{rel.to_function}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if interfaces.length > 0}
        <div class="interfaces-section">
          <h3>Interfaces</h3>
          <div class="interface-grid">
            {#each interfaces as iface}
              <div class="interface-card">
                <span class="iface-name">{iface.name}</span>
                <span class="iface-funcs">{iface.functions.length} functions</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .map-view {
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
  .logo { font-size: 18px; font-weight: 700; color: #f0f6fc; }
  .subtitle { font-size: 12px; color: #484f58; }
  .stats { font-size: 12px; color: #8b949e; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .tool-btn {
    background: #21262d; border: 1px solid #30363d; color: #c9d1d9;
    padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  .tool-btn:hover { border-color: #58a6ff; }

  .error { padding: 24px; color: #f85149; }
  .loading { padding: 24px; color: #8b949e; }

  .canvas-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .contract-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 16px;
  }

  .contract-card {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 10px;
    overflow: hidden;
  }

  .card-header {
    padding: 12px 14px 8px;
    border-bottom: 1px solid #21262d;
  }
  .card-kind { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }
  .card-header h2 { font-size: 18px; margin: 2px 0 0; }
  .card-header h2 a { color: #f0f6fc; }
  .card-header h2 a:hover { color: #58a6ff; text-decoration: none; }
  .card-inherits { font-size: 11px; color: #484f58; font-style: italic; margin-top: 2px; }

  .card-section { padding: 8px 14px; }
  .card-section + .card-section { border-top: 1px solid #21262d; }

  .section-title {
    font-size: 9px; color: #8b949e; text-transform: uppercase;
    letter-spacing: 0.5px; margin-bottom: 4px; font-weight: 600;
  }

  .func-item {
    display: flex; align-items: center; gap: 6px;
    padding: 4px 4px; border-radius: 4px;
    font-size: 12px; color: inherit;
  }
  .func-item:hover { background: #21262d; text-decoration: none; }
  .func-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .func-name { color: #f0f6fc; font-weight: 600; font-family: monospace; flex: 1; }
  .func-vis { font-size: 10px; color: #484f58; }
  .func-badge { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .func-badge.ext { background: #9e6a031a; color: #d29922; }
  .func-paths { font-size: 10px; color: #8b949e; display: flex; gap: 2px; }
  .func-paths .g { color: #3fb950; }
  .func-paths .r { color: #f85149; }

  .var-item {
    display: flex; justify-content: space-between;
    padding: 2px 4px; font-size: 11px; font-family: monospace;
  }
  .var-name { color: #c9d1d9; }
  .var-type { color: #484f58; font-size: 10px; max-width: 150px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .rel-item {
    display: flex; align-items: center; gap: 4px;
    padding: 2px 4px; font-size: 11px; font-family: monospace;
  }
  .rel-func { color: #c9d1d9; }
  .rel-arrow { color: #f85149; }
  .rel-target { color: #f85149; }

  .interfaces-section {
    margin-top: 24px;
  }
  .interfaces-section h3 { font-size: 14px; color: #8b949e; margin: 0 0 8px; }
  .interface-grid { display: flex; gap: 8px; flex-wrap: wrap; }
  .interface-card {
    background: #161b22; border: 1px dashed #30363d;
    border-radius: 6px; padding: 8px 12px;
    display: flex; gap: 8px; align-items: center;
  }
  .iface-name { color: #8b949e; font-weight: 600; font-size: 13px; }
  .iface-funcs { color: #484f58; font-size: 11px; }
</style>
