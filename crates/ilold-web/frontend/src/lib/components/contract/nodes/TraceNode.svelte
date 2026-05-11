<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { TraceNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: TraceNodeData } = $props();

  let hasError = $derived(data.error != null && data.error !== '');
</script>

<div
  class="trace-node py-1.5 px-3 rounded-md bg-surface-alt border-[1.5px] text-text font-mono text-xs min-w-[140px] text-center cursor-pointer"
  class:dimmed={data._dimmed}
  class:errored={hasError}
>
  <div class="flex items-center justify-center gap-1">
    <span class="text-[8px] text-text-dim">#{data.stepIndex}</span>
    <span class="font-semibold">{data.instruction}</span>
  </div>
  <div class="flex items-center justify-center gap-1 mt-0.5">
    <span class="text-[8px] text-text-dim">{data.computeUnits} CU</span>
    <span class="text-[8px] text-text-dim">{data.diffsCount} diffs</span>
    {#if hasError}
      <span class="text-[8px] px-1 rounded bg-danger/15 text-danger">err</span>
    {/if}
  </div>
</div>
<Handle type="target" id="t" position={Position.Top} />
<Handle type="source" id="b" position={Position.Bottom} />
<Handle type="target" id="l" position={Position.Left} />
<Handle type="source" id="r" position={Position.Right} />

<style>
  .trace-node {
    border-color: var(--color-accent-hover);
  }
  .trace-node.errored {
    border-color: var(--color-danger);
  }
  .trace-node.dimmed {
    opacity: 0.55;
  }
</style>
