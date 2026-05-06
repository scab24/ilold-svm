<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { InstructionNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: InstructionNodeData } = $props();

  let hasSigners = $derived((data.signers?.length ?? 0) > 0);
</script>

<div
  class="instruction-node py-1.5 px-4 rounded-md bg-surface-alt border-[1.5px] border-accent text-text font-mono text-xs font-semibold min-w-[120px] text-center cursor-pointer"
  class:dimmed={data._dimmed}
  class:has-pdas={data.hasPdas}
>
  <span>{data.label}</span>
  <div class="flex items-center justify-center gap-1 mt-0.5">
    <span class="text-[8px] text-text-dim">{data.argsCount}a · {data.accountsCount}acc</span>
    {#if data.hasPdas}
      <span class="text-[8px] px-1 rounded bg-warning/15 text-warning">PDA</span>
    {/if}
    {#if hasSigners}
      <span class="text-[9px]" title={`Signers: ${data.signers.join(', ')}`}>&#x1F511;</span>
    {/if}
  </div>
</div>
<Handle type="target" id="t" position={Position.Top} />
<Handle type="source" id="b" position={Position.Bottom} />
<Handle type="target" id="l" position={Position.Left} />
<Handle type="source" id="r" position={Position.Right} />

<style>
  .instruction-node.has-pdas {
    border-color: var(--color-warning);
  }
  .instruction-node.dimmed {
    opacity: 0.55;
  }
</style>
