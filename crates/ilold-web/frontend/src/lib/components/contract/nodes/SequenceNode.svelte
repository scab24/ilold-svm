<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { SequenceNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: SequenceNodeData } = $props();

  let hasConditions = $derived((data._transition?.conditions?.length ?? 0) > 0);
  let hasShared = $derived((data._transition?.shared_state?.length ?? 0) > 0);
</script>

<div
  class="seq-node py-1 px-2.5 rounded-sm bg-surface-alt border-[1.5px] border-accent text-text font-mono text-[11px] min-w-[80px] text-center flex items-center gap-1.5"
  class:readonly={data.readOnly}
  class:has-conditions={hasConditions}
  class:has-shared={hasShared}
  class:is-branch={data._isBranch}
  class:dimmed={data._dimmed}
>
  <span>{data.label}</span>
  {#if data.pathCount}
    <span class="text-[9px] text-text-dim">{data.pathCount}p</span>
  {/if}
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .seq-node.readonly {
    border-color: var(--color-text-muted);
    color: var(--color-text-muted);
  }
  .seq-node.has-conditions {
    background: var(--color-tint-warning);
    border-color: var(--color-warning);
    border-width: 2px;
  }
  .seq-node.has-shared {
    border-style: dashed;
  }
  .seq-node.is-branch {
    border-color: var(--color-success);
  }
  .seq-node.dimmed {
    opacity: 0.25;
    pointer-events: none;
  }
</style>
