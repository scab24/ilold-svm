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
  <div class="fixed z-60 bg-surface border border-border rounded-lg p-1 min-w-[160px] shadow-[0_8px_32px_var(--color-shadow)] backdrop-blur-md" style="left:{menu.x}px;top:{menu.y}px">
    {#if menu.nodeType === 'function'}
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
        {expandedFuncs.has(menu.funcName) ? '▼ Collapse CFG' : '▶ Expand CFG'}
      </button>
      {#if mode === 'sequences'}
        <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={() => onaddbranch(menu!.x, menu!.y, menu!.nodeId, menu!.funcName)}>
          + Add branch
        </button>
      {/if}
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-danger/5 hover:text-danger" onclick={() => onremovefunc(menu!.funcName)}>
        ✕ Remove from canvas
      </button>
    {:else if menu.nodeType === 'seq-next'}
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={() => onaddbranch(menu!.x, menu!.y, menu!.nodeId, menu!.funcName)}>
        + Add branch
      </button>
      {#if seqExpanded.has(menu.nodeId)}
        <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
          ▼ Collapse
        </button>
      {/if}
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-danger/5 hover:text-danger" onclick={() => onremovenode(menu!.nodeId)}>
        ✕ Remove node
      </button>
    {:else if menu.nodeType === 'block'}
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={() => onexpandcfg(menu!.funcName, menu!.nodeId)}>
        ▼ Collapse CFG
      </button>
      <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-danger/5 hover:text-danger" onclick={() => onremovefunc(menu!.funcName)}>
        ✕ Remove function
      </button>
    {/if}
    <button class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs cursor-pointer rounded-sm text-left font-[inherit] hover:bg-hover hover:text-accent-hover" onclick={onclose}>Cancel</button>
  </div>
{/if}
