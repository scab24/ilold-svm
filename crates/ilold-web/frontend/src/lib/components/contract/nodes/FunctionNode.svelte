<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { FunctionNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: FunctionNodeData } = $props();
</script>

<div
  class="function-node py-1.5 px-4 rounded-md bg-surface-alt border-[1.5px] border-accent text-text font-mono text-xs font-semibold min-w-[100px] text-center cursor-pointer"
  class:external={data.is_external}
  class:dimmed={data._dimmed}
>
  <span>{data.label}</span>
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .function-node.external {
    border-style: dashed;
    border-color: var(--color-danger);
    color: var(--color-danger-light);
    font-size: 11px;
  }
  .function-node.dimmed {
    opacity: 0.55;
  }
</style>
