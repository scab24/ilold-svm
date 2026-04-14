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

<div
  class="flex flex-col shrink-0 transition-[width] duration-200 {sidebarOpen ? 'w-[180px]' : 'w-8'}"
  style="
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.9) 0%, rgba(20, 20, 28, 0.95) 100%);
    border-right: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
  "
>
  <!-- Header — glass effect -->
  <div
    class="flex items-center justify-between px-2.5 py-2"
    style="
      border-bottom: 1px solid color-mix(in srgb, var(--color-border) 40%, transparent);
      background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
      backdrop-filter: blur(16px) saturate(1.8);
      -webkit-backdrop-filter: blur(16px) saturate(1.8);
    "
  >
    {#if sidebarOpen}
      <span class="text-[10px] text-text-muted uppercase tracking-wider font-semibold">Functions</span>
    {/if}
    <button
      class="bg-transparent border-none text-text-muted cursor-pointer text-[11px] px-1 py-0.5 transition-colors duration-150 hover:text-accent-hover"
      style="border-radius: 6px;"
      onclick={() => sidebarOpen = !sidebarOpen}
    >{sidebarOpen ? '◂' : '▸'}</button>
  </div>

  {#if sidebarOpen}
    <div class="flex-1 overflow-y-auto p-1.5">
      {#each entryPoints as func}
        {@const onCanvas = canvasFuncs.has(func.name)}
        <button
          class="flex items-center gap-1.5 w-full px-2 py-[6px] bg-transparent border-none text-text-muted text-[11px] font-mono cursor-pointer text-left transition-colors duration-150 hover:text-text {onCanvas ? 'text-accent-hover' : ''}"
          style="
            border-radius: 6px;
            background: {onCanvas ? 'color-mix(in srgb, var(--color-accent) 8%, transparent)' : 'transparent'};
          "
          onclick={() => onCanvas ? onremove(func.name) : onadd(func.name)}
          title={onCanvas ? 'Remove from canvas' : 'Add to canvas'}
        >
          <span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{func.name}</span>
          <span
            class="text-[9px] text-text-dim px-1.5 py-px font-mono"
            style="border-radius: 8px; background: color-mix(in srgb, var(--color-border) 40%, transparent);"
          >{func.path_count}p</span>
          {#if onCanvas}
            <span
              class="text-[10px]"
              style="color: var(--color-accent); text-shadow: 0 0 6px var(--color-accent);"
            >✓</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
