<script lang="ts">
  let { mode }: { mode: 'cfg' | 'sequences' } = $props();
</script>

<div class="legend">
  {#if mode === 'cfg'}
    <span><span class="dot" style="background:#5b9bd5"></span>Function</span>
    <span><span class="dot" style="background:#3a6b9f"></span>Entry block</span>
    <span><span class="dot" style="background:#5a9a6a"></span>Return</span>
    <span><span class="dot" style="background:#b05050"></span>Revert</span>
    <span>Click → expand CFG</span>
  {:else}
    <span><span class="dot" style="background:#5b9bd5"></span>State-changing</span>
    <span><span class="dot" style="border:1px solid #6b7a8d;background:transparent"></span>Read-only</span>
    <span><span class="dot" style="background:#c49a4a"></span>Conditions affected</span>
    <span>Click → expand · Shift+click → add branch</span>
  {/if}
</div>

<style>
  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px; font-size: 11px; color: #6b7a8d;
    background: #18181edd; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #252530; z-index: 10;
    backdrop-filter: blur(8px);
  }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }
</style>
