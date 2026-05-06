<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { AccountNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: AccountNodeData } = $props();

  let fieldCount = $derived(data.fields?.length ?? 0);
</script>

<div
  class="account-node py-1.5 px-3 rounded-md bg-surface-alt border-[1.5px] text-text font-mono text-xs min-w-[100px] text-center cursor-pointer"
  class:dimmed={data._dimmed}
>
  <div class="font-semibold">{data.label}</div>
  {#if fieldCount > 0}
    <div class="text-[8px] text-text-dim mt-0.5">{fieldCount} fields</div>
  {/if}
</div>
<Handle type="target" id="t" position={Position.Top} />
<Handle type="source" id="b" position={Position.Bottom} />
<Handle type="target" id="l" position={Position.Left} />
<Handle type="source" id="r" position={Position.Right} />

<style>
  .account-node {
    border-color: var(--color-success);
  }
  .account-node.dimmed {
    opacity: 0.55;
  }
</style>
