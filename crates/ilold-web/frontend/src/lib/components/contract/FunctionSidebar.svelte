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

  // Auditors only care about entry points — public + external functions.
  // Internal/private functions aren't callable from outside so they're noise here.
  const entryPoints = $derived(
    contract.functions.filter(
      (f) => f.visibility === 'Public' || f.visibility === 'External',
    ),
  );
</script>

<div class="flex flex-col shrink-0 bg-surface border-r border-border transition-[width] duration-200 {sidebarOpen ? 'w-[180px]' : 'w-8'}">
  <div class="flex items-center justify-between px-2 py-1.5 border-b border-border">
    {#if sidebarOpen}
      <span class="text-[10px] text-text-muted uppercase tracking-wide font-semibold">Functions</span>
    {/if}
    <button
      class="bg-transparent border-none text-text-muted cursor-pointer text-[11px] px-1 py-0.5 hover:text-accent-hover"
      onclick={() => sidebarOpen = !sidebarOpen}
    >{sidebarOpen ? '◂' : '▸'}</button>
  </div>
  {#if sidebarOpen}
    <div class="flex-1 overflow-y-auto p-1">
      {#each entryPoints as func}
        {@const onCanvas = canvasFuncs.has(func.name)}
        <button
          class="flex items-center gap-1 w-full px-1.5 py-[5px] bg-transparent border-none text-text-muted text-[11px] font-mono cursor-pointer rounded-sm text-left hover:bg-hover hover:text-text {onCanvas ? 'text-accent-hover' : ''}"
          onclick={() => onCanvas ? onremove(func.name) : onadd(func.name)}
          title={onCanvas ? 'Remove from canvas' : 'Add to canvas'}
        >
          <span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{func.name}</span>
          <span class="text-[9px] text-text-dim">{func.path_count}p</span>
          {#if onCanvas}<span class="text-accent text-[10px]">✓</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
