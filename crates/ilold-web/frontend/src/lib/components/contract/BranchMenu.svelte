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
  <div class="fixed z-60 bg-surface border border-border rounded-lg p-1 shadow-[0_8px_32px_var(--color-shadow)] backdrop-blur-md max-h-[280px] overflow-y-auto min-w-[160px]" style="left:{menu.x}px;top:{menu.y}px">
    <div class="text-[10px] text-text-muted px-2 py-1 font-semibold uppercase tracking-wide">Branch from {menu.parentFuncName}</div>
    {#each functions as f}
      <button
        class="flex items-center gap-1.5 w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs font-mono cursor-pointer rounded-sm text-left hover:bg-hover hover:text-accent-hover"
        onclick={() => onselect(menu!.parentNodeId, menu!.parentFuncName, f.name)}
      >
        {f.name}
        {#if f.read_only}<span class="text-[9px] text-text-muted bg-border px-[5px] py-px rounded-full">view</span>{/if}
      </button>
    {/each}
    <button class="block w-full px-2.5 py-[5px] mt-0.5 bg-transparent border-0 border-t border-border-subtle text-text-dim text-[11px] cursor-pointer text-center hover:text-danger" onclick={onclose}>Cancel</button>
  </div>
{/if}
