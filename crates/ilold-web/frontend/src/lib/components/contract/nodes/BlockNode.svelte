<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { BlockNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: BlockNodeData } = $props();

  let typeClass = $derived(
    data.node_type === 'Entry' ? 'entry'
    : data.node_type === 'Return' ? 'return'
    : data.node_type === 'Revert' ? 'revert'
    : data.node_type === 'LoopCondition' ? 'loop'
    : 'default'
  );
</script>

<div
  class="block-node py-1 px-3 rounded-sm bg-hover border border-border font-mono text-[10px] min-w-[80px] text-center"
  class:entry={typeClass === 'entry'}
  class:return={typeClass === 'return'}
  class:revert={typeClass === 'revert'}
  class:loop={typeClass === 'loop'}
  class:dimmed={data._dimmed}
>
  <span>{data.label}</span>
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .block-node { color: var(--color-text-muted); }
  .block-node.entry {
    background: var(--color-tint-accent);
    border-color: var(--color-accent-dark);
    color: var(--color-accent-hover);
  }
  .block-node.return {
    background: var(--color-tint-success);
    border-color: var(--color-success);
    color: var(--color-success-light);
  }
  .block-node.revert {
    background: var(--color-tint-danger);
    border-color: var(--color-danger);
    color: var(--color-danger-light);
  }
  .block-node.loop {
    background: var(--color-tint-warning);
    border-color: var(--color-warning);
    color: var(--color-warning);
    border-radius: 50%;
    padding: 8px;
  }
  .block-node.dimmed {
    opacity: 0.2;
  }
</style>
