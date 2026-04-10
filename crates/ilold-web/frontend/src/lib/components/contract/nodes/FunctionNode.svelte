<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { FunctionNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: FunctionNodeData } = $props();
</script>

<div class="function-node" class:external={data.is_external} class:dimmed={data._dimmed}>
  <span>{data.label}</span>
</div>
<Handle type="target" position={Position.Top} />
<Handle type="source" position={Position.Bottom} />

<style>
  .function-node {
    padding: 6px 16px;
    border-radius: 6px;
    background: #1a1a22;
    border: 1.5px solid #5b9bd5;
    color: #b8c4d4;
    font-size: 12px;
    font-family: monospace;
    font-weight: 600;
    min-width: 100px;
    text-align: center;
    cursor: pointer;
  }
  .function-node.external {
    border-style: dashed;
    border-color: #b05050;
    color: #c07070;
    font-size: 11px;
  }
  .function-node.dimmed {
    opacity: 0.55;
  }
</style>
