<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { ContractNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: ContractNodeData } = $props();

  let kindClass = $derived(
    data.kind === 'interface' ? 'kind-interface'
    : data.kind === 'library' ? 'kind-library'
    : data.kind === 'abstract' ? 'kind-abstract'
    : 'kind-contract'
  );
</script>

<div
  class="contract-node py-2 px-4 rounded-md bg-surface-alt text-text font-mono text-xs font-semibold min-w-[130px] text-center cursor-pointer {kindClass}"
  class:dimmed={data._dimmed}
  style="border-color: {data.color ?? 'var(--color-accent)'};"
>
  <span class="text-[13px]">{data.label}</span>
  <div class="flex items-center justify-center gap-1.5 mt-0.5">
    <span class="text-[8px] uppercase tracking-wide text-text-muted">{data.kind}</span>
    {#if data.folder}
      <span class="text-[8px] text-text-dim">{data.folder}</span>
    {/if}
  </div>
</div>
<Handle type="target" id="t" position={Position.Top} />
<Handle type="source" id="b" position={Position.Bottom} />
<Handle type="target" id="l" position={Position.Left} />
<Handle type="source" id="r" position={Position.Right} />

<style>
  .contract-node { border-width: 1.5px; border-style: solid; }
  .contract-node.kind-abstract { border-style: dashed; }
  .contract-node.kind-interface { border-style: dotted; color: var(--color-text-muted); }
  .contract-node.kind-library { border-width: 3px; border-style: double; }
  .contract-node.dimmed { opacity: 0.35; }
</style>
