<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { InstructionNodeData } from '$lib/stores/graph.svelte';
  import {
    getCallsPerIx,
    getCuStatsPerIx,
    getFailedPerIx,
  } from '$lib/stores/runtimeOverlay.svelte';

  let { data }: { data: InstructionNodeData } = $props();

  const MAX_ARGS = 4;
  let args = $derived(data.args ?? []);
  let visibleArgs = $derived(args.slice(0, MAX_ARGS));
  let extraArgs = $derived(Math.max(0, args.length - MAX_ARGS));
  let signerCount = $derived(data.signers?.length ?? 0);
  let discriminatorShort = $derived(
    data.discriminator_hex && data.discriminator_hex.length > 10
      ? `${data.discriminator_hex.slice(0, 10)}...`
      : data.discriminator_hex ?? '',
  );
  let discriminatorTooltip = $derived(
    data.discriminator_hex ? `discriminator: ${data.discriminator_hex}` : '',
  );
  let nodeTitle = $derived(
    data.adminGated
      ? `admin-gated (heuristic)${discriminatorTooltip ? ' · ' + discriminatorTooltip : ''}`
      : discriminatorTooltip,
  );

  let callsCount = $derived(getCallsPerIx()[data.label] ?? 0);
  let failedCount = $derived(getFailedPerIx()[data.label] ?? 0);
  let cuStats = $derived(getCuStatsPerIx()[data.label]);
  let cuLabel = $derived(cuStats ? formatCu(cuStats.avg) : '');

  function formatCu(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
    return `${n}`;
  }
</script>

<div
  class="instruction-node py-1.5 px-3 rounded-md bg-surface-alt border-[1.5px] border-accent text-text font-mono text-xs min-w-[140px] cursor-pointer"
  class:dimmed={data._dimmed}
  class:has-pdas={data.hasPdas}
  class:admin-gated={data.adminGated}
  title={nodeTitle}
>
  <div class="head">
    <span class="label">{data.label}</span>
    {#if discriminatorShort}
      <span class="disc">{discriminatorShort}</span>
    {/if}
  </div>

  {#if visibleArgs.length > 0}
    <ul class="arg-list mt-1">
      {#each visibleArgs as arg (arg.name)}
        <li class="arg-row">
          <span class="arg-name">{arg.name}</span>
          <span class="arg-type">{arg.ty}</span>
        </li>
      {/each}
      {#if extraArgs > 0}
        <li class="arg-more">+{extraArgs} more</li>
      {/if}
    </ul>
  {/if}

  <div class="badges mt-1">
    <span class="badge meta">{data.accountsCount}acc</span>
    {#if signerCount > 0}
      <span class="badge signer" title={`Signers: ${data.signers.join(', ')}`}>
        {signerCount} signer{signerCount > 1 ? 's' : ''}
      </span>
    {/if}
    {#if data.hasPdas}
      <span class="badge pda">pda</span>
    {/if}
    {#if data.adminGated}
      <span class="badge admin">admin</span>
    {/if}
    {#if callsCount > 0}
      <span class="badge runtime-calls" title={`called ${callsCount} time${callsCount > 1 ? 's' : ''}`}>
        called {callsCount}x
      </span>
    {/if}
    {#if cuLabel}
      <span class="badge runtime-cu" title={cuStats ? `min ${cuStats.min} · avg ${cuStats.avg} · max ${cuStats.max} (${cuStats.samples} samples)` : ''}>
        ~{cuLabel} CU
      </span>
    {/if}
    {#if failedCount > 0}
      <span class="badge runtime-failed" title={`rejected ${failedCount} time${failedCount > 1 ? 's' : ''}`}>
        rejected {failedCount}x
      </span>
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
  .instruction-node.admin-gated {
    border-color: var(--color-danger);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-danger) 35%, transparent) inset;
  }
  .instruction-node.dimmed {
    opacity: 0.55;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 6px;
  }
  .label {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .disc {
    font-size: 8px;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
  }
  .arg-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .arg-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 9px;
    line-height: 1.3;
  }
  .arg-name {
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arg-type {
    color: var(--color-text-dim);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arg-more {
    font-size: 8px;
    color: var(--color-text-dim);
    text-align: center;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    justify-content: center;
  }
  .badge {
    font-size: 8px;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-border-subtle);
    color: var(--color-text-muted);
    text-transform: lowercase;
    letter-spacing: 0.04em;
  }
  .badge.signer {
    background: color-mix(in srgb, var(--color-accent) 18%, transparent);
    color: var(--color-accent-hover);
  }
  .badge.pda {
    background: color-mix(in srgb, var(--color-warning) 22%, transparent);
    color: var(--color-warning);
  }
  .badge.admin {
    background: color-mix(in srgb, var(--color-danger) 22%, transparent);
    color: var(--color-danger);
  }
  .badge.runtime-calls {
    background: color-mix(in srgb, var(--color-success, #4ade80) 22%, transparent);
    color: var(--color-success, #4ade80);
  }
  .badge.runtime-cu {
    background: color-mix(in srgb, var(--color-info, #60a5fa) 18%, transparent);
    color: var(--color-info, #60a5fa);
  }
  .badge.runtime-failed {
    background: color-mix(in srgb, var(--color-danger) 28%, transparent);
    color: var(--color-danger);
  }
</style>
