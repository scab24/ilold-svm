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

<div class="block-node {typeClass}">
  <span>{data.label}</span>
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .block-node {
    padding: 4px 12px;
    border-radius: 4px;
    background: #1e1e28;
    border: 1px solid #252530;
    color: #8b95a5;
    font-size: 10px;
    font-family: monospace;
    min-width: 80px;
    text-align: center;
  }
  .block-node.entry {
    background: #1a2a3a;
    border-color: #3a6b9f;
    color: #8bb8e8;
  }
  .block-node.return {
    background: #1a2a1a;
    border-color: #5a9a6a;
    color: #7aba8a;
  }
  .block-node.revert {
    background: #2a1a1a;
    border-color: #b05050;
    color: #c07070;
  }
  .block-node.loop {
    background: #2a2518;
    border-color: #c49a4a;
    color: #c49a4a;
    border-radius: 50%;
    padding: 8px;
  }
</style>
