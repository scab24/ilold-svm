<script lang="ts">
  let { mode }: { mode: 'cfg' | 'sequences' } = $props();
</script>

<div class="fixed bottom-3 left-4 flex gap-2.5 text-[11px] text-text-muted bg-surface/95 px-3 py-1.5 rounded-md border border-border z-10 backdrop-blur-md">
  {#if mode === 'cfg'}
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-accent"></span>Function</span>
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-accent-dark"></span>Entry block</span>
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-success"></span>Return</span>
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-danger"></span>Revert</span>
    <span>Click → expand CFG</span>
  {:else}
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-accent"></span>State-changing</span>
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] border border-text-muted bg-transparent"></span>Read-only</span>
    <span><span class="inline-block w-2 h-2 rounded-[2px] align-middle mr-[3px] bg-warning"></span>Conditions affected</span>
    <span>Click → expand · Shift+click → add branch</span>
  {/if}
</div>
