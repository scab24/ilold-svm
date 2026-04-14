<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import type { FunctionNodeData } from '$lib/stores/graph.svelte';

  let { data }: { data: FunctionNodeData } = $props();

  let visLabel = $derived(
    data.visibility === 'Public' ? 'pub'
    : data.visibility === 'External' ? 'ext'
    : data.visibility === 'Internal' ? 'int'
    : data.visibility === 'Private' ? 'priv'
    : null
  );

  let visClass = $derived(
    data.visibility === 'Public' ? 'bg-accent-dark/30 text-accent-hover'
    : data.visibility === 'External' ? 'bg-warning/20 text-warning'
    : data.visibility === 'Internal' ? 'bg-border text-text-muted'
    : data.visibility === 'Private' ? 'bg-border text-text-muted'
    : ''
  );

  let mutIcon = $derived(
    data.mutability === 'View' || data.mutability === 'Pure' ? '\u{1F441}'
    : data.mutability === 'Payable' ? '\u{1F4B0}'
    : null
  );

  let hasAccessControl = $derived(
    (data.modifiers?.length ?? 0) > 0
  );

  let hasBadges = $derived(
    visLabel != null || mutIcon != null || (data.path_count != null && data.path_count > 0) || hasAccessControl
  );

  // ── Scenario styling (Phase S5) ────────────────────────────────────────────
  // `_divergenceCount` appears on the last shared-prefix node when >1 scenarios
  // diverge past it. `_scenario` is set on divergent tail nodes and drives the
  // active-glow / muted-opacity pair below.
  let divergenceCount = $derived(
    typeof data._divergenceCount === 'number' && data._divergenceCount > 1
      ? data._divergenceCount
      : null
  );
  let scenarioName = $derived(data._scenario ?? null);
  let scenarioActive = $derived(
    scenarioName != null && data._activeScenario === scenarioName
  );
  let scenarioMuted = $derived(
    scenarioName != null && data._activeScenario !== scenarioName
  );
</script>

<div
  class="function-node py-1.5 px-4 rounded-md bg-surface-alt border-[1.5px] border-accent text-text font-mono text-xs font-semibold min-w-[100px] text-center cursor-pointer"
  class:external={data.is_external}
  class:dimmed={data._dimmed}
  class:scenario-active={scenarioActive}
  class:scenario-muted={scenarioMuted}
>
  <span>{data.label}</span>
  {#if scenarioName}
    <span
      class="inline-block ml-1 text-[8px] px-1 rounded bg-accent-dark/30 text-accent-hover align-middle"
      title={`Scenario: ${scenarioName}`}
    >{scenarioName}</span>
  {/if}
  {#if divergenceCount}
    <span
      class="inline-block ml-1 text-[8px] px-1 rounded bg-warning/20 text-warning align-middle"
      title={`${divergenceCount} scenarios diverge here`}
    >{divergenceCount}⑃</span>
  {/if}
  {#if hasBadges}
    <div class="flex items-center justify-center gap-1 mt-0.5">
      {#if visLabel}
        <span class="text-[8px] px-1 rounded {visClass}">{visLabel}</span>
      {/if}
      {#if mutIcon}
        <span class="text-[9px]">{mutIcon}</span>
      {/if}
      {#if data.path_count != null && data.path_count > 0}
        <span class="text-[8px] text-text-dim">{data.path_count}p</span>
      {/if}
      {#if hasAccessControl}
        <span class="text-[9px]" title={data.modifiers?.join(', ')}>&#x1F512;</span>
      {/if}
    </div>
  {/if}
</div>
<Handle type="target" id="t" position={Position.Top} />
<Handle type="source" id="b" position={Position.Bottom} />
<Handle type="target" id="l" position={Position.Left} />
<Handle type="source" id="r" position={Position.Right} />

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
  /* Phase S5 — scenario highlight/mute. Uses --color-accent from the token set
     with a soft fallback so the glow stays visible even if the variable is
     missing in a future theme refactor. */
  .function-node.scenario-active {
    box-shadow: 0 0 8px var(--color-accent, rgba(88, 166, 255, 0.4));
  }
  .function-node.scenario-muted {
    opacity: 0.7;
  }
</style>
