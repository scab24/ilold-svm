<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { SequenceNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: SequenceNodeData } = $props();

  let hasConditions = $derived((data._transition?.conditions?.length ?? 0) > 0);
  let hasShared = $derived((data._transition?.shared_state?.length ?? 0) > 0);
</script>

<div
  class="seq-node"
  class:readonly={data.readOnly}
  class:has-conditions={hasConditions}
  class:has-shared={hasShared}
  class:is-branch={data._isBranch}
>
  <span>{data.label}</span>
  {#if data.pathCount}
    <span class="sn-paths">{data.pathCount}p</span>
  {/if}
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .seq-node {
    padding: 4px 10px;
    border-radius: 4px;
    background: #1a1a22;
    border: 1.5px solid #5b9bd5;
    color: #b8c4d4;
    font-size: 11px;
    font-family: monospace;
    min-width: 80px;
    text-align: center;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .seq-node.readonly {
    border-color: #6b7a8d;
    color: #6b7a8d;
  }
  .seq-node.has-conditions {
    background: #2a2518;
    border-color: #c49a4a;
    border-width: 2px;
  }
  .seq-node.has-shared {
    border-style: dashed;
  }
  .seq-node.is-branch {
    border-color: #5a9a6a;
  }
  .sn-paths {
    font-size: 9px;
    color: #4a5568;
  }
</style>
