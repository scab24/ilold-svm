<script lang="ts">
  let { children, title = '', open = false, count }: {
    children: any;
    title?: string;
    open?: boolean;
    count?: number;
  } = $props();

  let isOpen = $state(open);
</script>

<div class="collapsible">
  <button class="coll-header" onclick={() => isOpen = !isOpen}>
    <span class="coll-arrow">{isOpen ? '▼' : '▶'}</span>
    <span class="coll-title">{title}</span>
    {#if count !== undefined}
      <span class="coll-count">{count}</span>
    {/if}
  </button>
  {#if isOpen}
    <div class="coll-body">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .collapsible { margin: 4px 0; }
  .coll-header {
    display: flex; align-items: center; gap: 4px; width: 100%;
    background: none; border: none; padding: 4px 2px;
    cursor: pointer; font: inherit; text-align: left;
  }
  .coll-header:hover { background: #1e1e28; border-radius: 3px; }
  .coll-arrow { font-size: 8px; color: #4a5568; width: 12px; text-align: center; }
  .coll-title {
    font-size: 10px; color: #6b7a8d; text-transform: uppercase;
    letter-spacing: 0.5px; font-weight: 600; flex: 1;
  }
  .coll-count { font-size: 9px; color: #4a5568; }
  .coll-body { padding: 2px 0 2px 4px; }
</style>
