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
  <div
    class="fixed z-60 p-1.5 max-h-[280px] overflow-y-auto min-w-[170px]"
    style="
      left:{menu.x}px;
      top:{menu.y}px;
      border-radius: 10px;
      border: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
      background: linear-gradient(180deg, rgba(30, 30, 40, 0.88) 0%, rgba(24, 24, 30, 0.92) 100%);
      backdrop-filter: blur(16px) saturate(1.8);
      -webkit-backdrop-filter: blur(16px) saturate(1.8);
      box-shadow:
        0 12px 40px -8px rgba(0, 0, 0, 0.45),
        0 4px 16px -4px rgba(0, 0, 0, 0.25),
        0 0 0 1px rgba(91, 155, 213, 0.05);
    "
  >
    <div class="text-[10px] text-text-muted px-2.5 py-1.5 font-semibold uppercase tracking-wider">Branch from {menu.parentFuncName}</div>
    {#each functions as f}
      <button
        class="flex items-center gap-1.5 w-full px-2.5 py-1.5 bg-transparent border-none text-text text-xs font-mono cursor-pointer text-left transition-colors duration-150 hover:text-accent-hover"
        style="border-radius: 6px;"
        onclick={() => onselect(menu!.parentNodeId, menu!.parentFuncName, f.name)}
      >
        {f.name}
        {#if f.read_only}
          <span
            class="text-[9px] text-text-muted px-1.5 py-px"
            style="border-radius: 8px; background: color-mix(in srgb, var(--color-border) 40%, transparent);"
          >view</span>
        {/if}
      </button>
    {/each}
    <!-- Separator before cancel -->
    <div class="my-1" style="height: 1px; background: color-mix(in srgb, var(--color-border) 30%, transparent);"></div>
    <button
      class="block w-full px-2.5 py-1.5 bg-transparent border-none text-text-muted text-[11px] cursor-pointer text-center transition-colors duration-150 hover:text-danger"
      style="border-radius: 6px;"
      onclick={onclose}
    >Cancel</button>
  </div>
{/if}
