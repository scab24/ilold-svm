<script lang="ts">
  interface Props {
    menu: { x: number; y: number; parentNodeId: string; parentFuncName: string } | null;
    functions: Array<{ name: string; read_only?: boolean }>;
    onselect: (parentNodeId: string, parentFuncName: string, branchFunc: string) => void;
    onclose: () => void;
  }

  let { menu, functions, onselect, onclose }: Props = $props();
</script>

{#if menu}
  <div class="branch-menu" style="left:{menu.x}px;top:{menu.y}px">
    <div class="branch-title">Branch from {menu.parentFuncName}</div>
    {#each functions as f}
      <button class="branch-item" onclick={() => onselect(menu!.parentNodeId, menu!.parentFuncName, f.name)}>
        {f.name}
        {#if f.read_only}<span class="branch-tag">view</span>{/if}
      </button>
    {/each}
    <button class="branch-close" onclick={onclose}>Cancel</button>
  </div>
{/if}

<style>
  .branch-menu {
    position: fixed; z-index: 60;
    background: #18181e; border: 1px solid #252530;
    border-radius: 8px; padding: 4px;
    box-shadow: 0 8px 32px #08080a88;
    backdrop-filter: blur(12px);
    max-height: 280px; overflow-y: auto;
    min-width: 160px;
  }
  .branch-title { font-size: 10px; color: #6b7a8d; padding: 4px 8px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }
  .branch-item {
    display: flex; align-items: center; gap: 6px; width: 100%;
    padding: 6px 10px; background: none; border: none;
    color: #b8c4d4; font-size: 12px; font-family: monospace;
    cursor: pointer; border-radius: 4px; text-align: left;
  }
  .branch-item:hover { background: #1e1e28; color: #8bb8e8; }
  .branch-tag { font-size: 9px; color: #6b7a8d; background: #252830; padding: 1px 5px; border-radius: 8px; }
  .branch-close {
    display: block; width: 100%; padding: 5px 10px; margin-top: 2px;
    background: none; border: none; border-top: 1px solid #2a2d38;
    color: #4a5568; font-size: 11px; cursor: pointer; text-align: center;
  }
  .branch-close:hover { color: #b05050; }
</style>
