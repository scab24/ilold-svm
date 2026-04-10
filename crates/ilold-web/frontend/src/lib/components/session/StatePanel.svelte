<script lang="ts">
  import { postCommand } from '$lib/api/session';
  import { getSteps } from '$lib/stores/session.svelte';

  interface Props {
    contract: string;
  }

  let { contract }: Props = $props();

  // ── Local state ────────────────────────────────────────────────────────────

  let stateVars = $state<any[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let expandedVar = $state<string | null>(null);

  // ── Fetch state on mount and when steps change ─────────────────────────────

  async function fetchState() {
    loading = true;
    error = null;
    try {
      const result: any = await postCommand('State', contract);
      // CommandResult::StateView → { StateView: { summary: [...] } }
      const view = result.StateView ?? result;
      stateVars = view.summary ?? [];
    } catch (e: any) {
      error = e.message ?? 'Failed to fetch state';
      stateVars = [];
    } finally {
      loading = false;
    }
  }

  // Track step count — refetch when it changes.
  // Plain variable (not $state) to avoid creating a reactive dependency
  // inside the effect that would cause an infinite re-run loop.
  let prevStepCount = -1;

  $effect(() => {
    const count = getSteps().length;
    if (count !== prevStepCount) {
      prevStepCount = count;
      fetchState();
    }
  });

  // ── Toggle inline expansion (uses changes[] from summary) ─────────────────

  function toggleTimeline(varName: string) {
    expandedVar = expandedVar === varName ? null : varName;
  }
</script>

<div class="flex flex-col bg-surface border border-border rounded-md overflow-hidden h-full">
  <!-- Header -->
  <div class="flex items-center justify-between px-2.5 py-2 border-b border-border flex-shrink-0">
    <span class="text-[10px] text-text-muted uppercase tracking-[0.5px] font-semibold">State Variables</span>
    <button
      class="bg-transparent border border-border text-text-muted cursor-pointer text-xs px-1.5 py-0.5 rounded-[3px] leading-none hover:text-accent-hover hover:border-accent disabled:opacity-40 disabled:cursor-default"
      onclick={fetchState}
      disabled={loading}
      title="Refresh state"
    >
      {loading ? '...' : '↻'}
    </button>
  </div>

  <!-- Body -->
  <div class="panel-body flex-1 overflow-y-auto p-1">
    {#if loading && stateVars.length === 0}
      <div class="text-[11px] text-text-dim py-4 px-2 text-center italic">Loading state...</div>
    {:else if error && stateVars.length === 0}
      <div class="text-[11px] text-danger py-3 px-2 text-center">{error}</div>
    {:else if stateVars.length === 0}
      <div class="text-[11px] text-text-dim py-4 px-2 text-center italic">No state changes yet. Explore functions to see variable mutations.</div>
    {:else}
      {#each stateVars as v}
        <button
          class="flex items-center gap-1.5 w-full px-2 py-1.5 bg-transparent border-none border-b border-hover text-text text-[11px] font-mono cursor-pointer text-left rounded-none transition-[background] duration-100 hover:bg-hover {expandedVar === v.variable ? 'bg-surface-alt border-l-2 border-l-accent' : ''}"
          onclick={() => toggleTimeline(v.variable)}
        >
          <span class="text-accent-hover font-semibold flex-shrink-0 max-w-[120px] overflow-hidden text-ellipsis whitespace-nowrap">{v.variable}</span>
          <span class="flex-1 text-[10px] text-text-muted overflow-hidden text-ellipsis whitespace-nowrap">
            {#if v.changes?.length > 0}
              {v.changes[v.changes.length - 1]}
            {/if}
          </span>
          <span class="text-[9px] text-text-dim flex-shrink-0 w-2.5 text-center">{expandedVar === v.variable ? '▾' : '▸'}</span>
        </button>

        {#if expandedVar === v.variable}
          <div class="py-1 px-2 pb-2 pl-4 border-l-2 border-border ml-2 mb-1">
            {#if v.changes?.length > 0}
              {#each v.changes as change, i}
                <div class="font-mono text-[10px] text-text-muted py-[3px] {i < v.changes.length - 1 ? 'border-b border-hover' : ''}">{change}</div>
              {/each}
            {:else}
              <div class="text-[10px] text-text-dim py-1.5 italic">No changes recorded.</div>
            {/if}
          </div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

<style>
  /* Scrollbar — pseudo-elements require scoped CSS */
  .panel-body::-webkit-scrollbar { width: 4px; }
  .panel-body::-webkit-scrollbar-track { background: transparent; }
  .panel-body::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 2px; }
</style>
