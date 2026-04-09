<script lang="ts">
  import type { ContractDetail } from '$lib/api/rest';

  let {
    contract,
    canvasFuncs,
    onadd,
    onremove,
  }: {
    contract: ContractDetail;
    canvasFuncs: Set<string>;
    onadd: (func: string) => void;
    onremove: (func: string) => void;
  } = $props();

  let sidebarOpen = $state(true);
</script>

<div class="sidebar" class:collapsed={!sidebarOpen}>
  <div class="sidebar-header">
    <span class="sidebar-title">Functions</span>
    <button class="sidebar-toggle" onclick={() => sidebarOpen = !sidebarOpen}>{sidebarOpen ? '◂' : '▸'}</button>
  </div>
  {#if sidebarOpen}
    <div class="sidebar-body">
      {#each contract.functions as func}
        {@const onCanvas = canvasFuncs.has(func.name)}
        <button
          class="sidebar-func"
          class:on-canvas={onCanvas}
          onclick={() => onCanvas ? onremove(func.name) : onadd(func.name)}
          title={onCanvas ? 'Remove from canvas' : 'Add to canvas'}
        >
          <span class="sf-name">{func.name}</span>
          <span class="sf-meta">{func.path_count}p</span>
          {#if onCanvas}<span class="sf-check">✓</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .sidebar {
    width: 180px; flex-shrink: 0;
    background: #18181e; border-right: 1px solid #252530;
    display: flex; flex-direction: column;
    transition: width 0.2s;
  }
  .sidebar.collapsed { width: 32px; }
  .sidebar-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 8px; border-bottom: 1px solid #252530;
  }
  .sidebar-title { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .collapsed .sidebar-title { display: none; }
  .sidebar-toggle {
    background: none; border: none; color: #6b7a8d; cursor: pointer;
    font-size: 11px; padding: 2px 4px;
  }
  .sidebar-toggle:hover { color: #8bb8e8; }
  .sidebar-body { flex: 1; overflow-y: auto; padding: 4px; }
  .sidebar-func {
    display: flex; align-items: center; gap: 4px; width: 100%;
    padding: 5px 6px; background: none; border: none;
    color: #6b7a8d; font-size: 11px; font-family: monospace;
    cursor: pointer; border-radius: 4px; text-align: left;
  }
  .sidebar-func:hover { background: #1e1e28; color: #b8c4d4; }
  .sidebar-func.on-canvas { color: #8bb8e8; }
  .sf-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sf-meta { font-size: 9px; color: #4a5568; }
  .sf-check { color: #5b9bd5; font-size: 10px; }
</style>
