<script lang="ts">
  import { onMount } from 'svelte';
  import { getProjectMap, type ProjectMap, type MapContract } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search.svelte';

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

  function mutColorClass(m: string): string {
    if (m === 'View' || m === 'Pure') return 'bg-accent';
    return 'bg-accent-hover';
  }
</script>

<div class="fixed inset-0 flex flex-col bg-dark">
  <div class="flex items-center gap-2.5 px-4 py-2 bg-hover border-b border-border-subtle z-10 shrink-0">
    <span class="text-lg font-bold text-text">ilold</span>
    <span class="text-xs text-text-dim">execution path analyzer</span>
    {#if projectMap}
      <span class="text-xs text-text-muted">{projectMap.contracts.length} contracts · {projectMap.relationships.length} cross-contract calls</span>
    {/if}
    <div class="ml-auto flex gap-1">
      <button class="bg-hover border border-border-subtle text-accent-hover px-3 py-1 rounded-sm cursor-pointer text-xs hover:border-accent" onclick={toggleSearch}>🔍 Search</button>
    </div>
  </div>

  {#if error}
    <div class="p-6 text-danger">{error}</div>
  {:else if !projectMap}
    <div class="p-6 text-text-muted">Analyzing...</div>
  {:else}
    <div class="flex-1 overflow-y-auto p-6">
      <div class="grid grid-cols-[repeat(auto-fill,minmax(340px,1fr))] gap-4">
        {#each contracts as contract}
          <div class="bg-hover border border-border-subtle rounded-[10px] overflow-hidden">
            <div class="px-3.5 pt-3 pb-2 border-b border-border-subtle">
              <span class="text-[10px] text-text-muted uppercase tracking-wide">{contract.kind.toLowerCase()}</span>
              <h2 class="text-lg mt-0.5 mb-0"><a class="text-text no-underline hover:text-accent-hover" href="/contract/{contract.name}">{contract.name}</a></h2>
              {#if contract.inherits.length > 0}
                <div class="text-[11px] text-text-dim italic mt-0.5">inherits {contract.inherits.join(', ')}</div>
              {/if}
            </div>

            <div class="card-section">
              <div class="text-[9px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Functions</div>
              {#each contract.functions as func}
                <a href="/contract/{contract.name}/{func.name}" class="flex items-center gap-1.5 px-1 py-1 rounded-sm text-xs text-inherit no-underline hover:bg-border">
                  <span class="size-1.5 rounded-full shrink-0 {mutColorClass(func.mutability)}"></span>
                  <span class="text-text font-semibold font-mono flex-1">{func.name}</span>
                  <span class="text-[10px] text-text-dim">{func.visibility.toLowerCase()}</span>
                  {#if func.has_external_calls}
                    <span class="text-[9px] px-1 py-px rounded-md bg-warning/10 text-warning">ext</span>
                  {/if}
                  <span class="text-[10px] text-text-muted flex gap-0.5">
                    {func.path_count}p
                    {#if func.happy_paths > 0}<span class="text-success">{func.happy_paths}✓</span>{/if}
                    {#if func.revert_paths > 0}<span class="text-danger">{func.revert_paths}✗</span>{/if}
                  </span>
                </a>
              {/each}
            </div>

            {#if contract.state_vars.length > 0}
              <div class="card-section">
                <div class="text-[9px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Variables</div>
                {#each contract.state_vars as sv}
                  <div class="flex justify-between px-1 py-0.5 text-[11px] font-mono">
                    <span class="text-text">{sv.name}</span>
                    <span class="text-text-dim text-[10px] max-w-[150px] overflow-hidden text-ellipsis whitespace-nowrap">{sv.type_name}</span>
                  </div>
                {/each}
              </div>
            {/if}

            {#if projectMap.relationships.filter(r => r.from_contract === contract.name).length > 0}
              <div class="card-section">
                <div class="text-[9px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Calls to</div>
                {#each projectMap.relationships.filter(r => r.from_contract === contract.name) as rel}
                  <div class="flex items-center gap-1 px-1 py-0.5 text-[11px] font-mono">
                    <span class="text-text">{rel.from_function}</span>
                    <span class="text-accent">→</span>
                    <span class="text-accent-hover">{rel.to_contract}.{rel.to_function}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if interfaces.length > 0}
        <div class="mt-6">
          <h3 class="text-sm text-text-muted mb-2">Interfaces</h3>
          <div class="flex gap-2 flex-wrap">
            {#each interfaces as iface}
              <div class="bg-hover border border-dashed border-border-subtle rounded-md px-3 py-2 flex gap-2 items-center">
                <span class="text-text-muted font-semibold text-[13px]">{iface.name}</span>
                <span class="text-text-dim text-[11px]">{iface.functions.length} functions</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .card-section { padding: 8px 14px; }
  .card-section + .card-section { border-top: 1px solid var(--color-border-subtle); }
</style>
