<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { AccountNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: AccountNodeData } = $props();

  const MAX_FIELDS = 6;
  let fields = $derived(data.fields ?? []);
  let visibleFields = $derived(fields.slice(0, MAX_FIELDS));
  let extraCount = $derived(Math.max(0, fields.length - MAX_FIELDS));
  let discriminatorTooltip = $derived(
    data.discriminator_hex ? `discriminator: ${data.discriminator_hex}` : '',
  );
</script>

<div
  class="account-node py-1.5 px-3 rounded-md bg-surface-alt border-[1.5px] text-text font-mono text-xs min-w-[120px] cursor-pointer"
  class:dimmed={data._dimmed}
  class:has-fields={fields.length > 0}
  title={discriminatorTooltip}
>
  <div class="head text-center">
    <div class="font-semibold">{data.label}</div>
    {#if data.account_type}
      <div class="text-[8px] text-text-dim mt-0.5 uppercase tracking-wider">{data.account_type}</div>
    {/if}
  </div>

  {#if visibleFields.length > 0}
    <ul class="field-list mt-1.5">
      {#each visibleFields as f (f.name)}
        <li class="field-row">
          <span class="field-name">{f.name}</span>
          <span class="field-type">{f.ty}</span>
        </li>
      {/each}
      {#if extraCount > 0}
        <li class="field-more">+{extraCount} more</li>
      {/if}
    </ul>
  {/if}

  {#if data.signer || data.writable || data.pda}
    <div class="badges">
      {#if data.signer}<span class="badge signer">signer</span>{/if}
      {#if data.writable}<span class="badge writable">writable</span>{/if}
      {#if data.pda}<span class="badge pda">pda</span>{/if}
    </div>
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
  .field-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .field-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 9px;
    line-height: 1.3;
  }
  .field-name {
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .field-type {
    color: var(--color-text-dim);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .field-more {
    font-size: 8px;
    color: var(--color-text-dim);
    text-align: center;
    margin-top: 2px;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    justify-content: center;
    margin-top: 4px;
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
  .badge.writable {
    background: color-mix(in srgb, var(--color-warning) 18%, transparent);
    color: var(--color-warning);
  }
  .badge.pda {
    background: color-mix(in srgb, var(--color-warning) 22%, transparent);
    color: var(--color-warning);
  }
</style>
