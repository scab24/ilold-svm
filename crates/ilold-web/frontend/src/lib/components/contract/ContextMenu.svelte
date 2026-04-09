<script lang="ts">
  interface Props {
    menu: { x: number; y: number; nodeId: string; funcName: string; nodeType: string } | null;
    expandedFuncs: Set<string>;
    seqExpanded: Map<string, boolean>;
    mode: 'cfg' | 'sequences';
    onexpandcfg: (funcName: string, nodeId: string) => void;
    onremovefunc: (funcName: string) => void;
    onremovenode: (nodeId: string) => void;
    onaddbranch: (x: number, y: number, nodeId: string, funcName: string) => void;
    onclose: () => void;
  }

  let { menu, expandedFuncs, seqExpanded, mode, onexpandcfg, onremovefunc, onremovenode, onaddbranch, onclose }: Props = $props();
</script>

{#if menu}
  <div class="ctx-menu" style="left:{menu.x}px;top:{menu.y}px">
    {#if menu.nodeType === 'function'}
      <button class="ctx-item" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
        {expandedFuncs.has(menu.funcName) ? '▼ Collapse CFG' : '▶ Expand CFG'}
      </button>
      {#if mode === 'sequences'}
        <button class="ctx-item" onclick={() => onaddbranch(menu!.x, menu!.y, menu!.nodeId, menu!.funcName)}>
          + Add branch
        </button>
      {/if}
      <button class="ctx-item ctx-danger" onclick={() => onremovefunc(menu!.funcName)}>
        ✕ Remove from canvas
      </button>
    {:else if menu.nodeType === 'seq-next'}
      <button class="ctx-item" onclick={() => onaddbranch(menu!.x, menu!.y, menu!.nodeId, menu!.funcName)}>
        + Add branch
      </button>
      {#if seqExpanded.has(menu.nodeId)}
        <button class="ctx-item" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
          ▼ Collapse
        </button>
      {/if}
      <button class="ctx-item ctx-danger" onclick={() => onremovenode(menu!.nodeId)}>
        ✕ Remove node
      </button>
    {:else if menu.nodeType === 'block'}
      <button class="ctx-item" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
        ▼ Collapse CFG
      </button>
      <button class="ctx-item ctx-danger" onclick={() => onremovefunc(menu!.funcName)}>
        ✕ Remove function
      </button>
    {/if}
    <button class="ctx-item" onclick={onclose}>Cancel</button>
  </div>
{/if}

<style>
  .ctx-menu {
    position: fixed; z-index: 60;
    background: #18181e; border: 1px solid #252530;
    border-radius: 8px; padding: 4px; min-width: 160px;
    box-shadow: 0 8px 32px #08080a88; backdrop-filter: blur(12px);
  }
  .ctx-item {
    display: block; width: 100%; padding: 6px 10px;
    background: none; border: none; color: #b8c4d4;
    font-size: 12px; cursor: pointer; border-radius: 4px;
    text-align: left; font-family: inherit;
  }
  .ctx-item:hover { background: #1e1e28; color: #8bb8e8; }
  .ctx-item.ctx-danger:hover { background: #b0505015; color: #b05050; }
</style>
