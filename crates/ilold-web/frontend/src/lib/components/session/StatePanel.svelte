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

<div class="state-panel">
  <div class="panel-header">
    <span class="panel-title">State Variables</span>
    <button class="refresh-btn" onclick={fetchState} disabled={loading} title="Refresh state">
      {loading ? '...' : '↻'}
    </button>
  </div>

  <div class="panel-body">
    {#if loading && stateVars.length === 0}
      <div class="state-loading">Loading state...</div>
    {:else if error && stateVars.length === 0}
      <div class="state-error">{error}</div>
    {:else if stateVars.length === 0}
      <div class="state-empty">No state changes yet. Explore functions to see variable mutations.</div>
    {:else}
      {#each stateVars as v}
        <button
          class="var-row"
          class:var-expanded={expandedVar === v.variable}
          onclick={() => toggleTimeline(v.variable)}
        >
          <span class="var-name">{v.variable}</span>
          <span class="var-changes">
            {#if v.changes?.length > 0}
              {v.changes[v.changes.length - 1]}
            {/if}
          </span>
          <span class="var-chevron">{expandedVar === v.variable ? '▾' : '▸'}</span>
        </button>

        {#if expandedVar === v.variable}
          <div class="timeline-section">
            {#if v.changes?.length > 0}
              {#each v.changes as change}
                <div class="tl-entry">{change}</div>
              {/each}
            {:else}
              <div class="tl-empty">No changes recorded.</div>
            {/if}
          </div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

<style>
  .state-panel {
    display: flex;
    flex-direction: column;
    background: #18181e;
    border: 1px solid #252530;
    border-radius: 6px;
    overflow: hidden;
    height: 100%;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-bottom: 1px solid #252530;
    flex-shrink: 0;
  }

  .panel-title {
    font-size: 10px;
    color: #6b7a8d;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 600;
  }

  .refresh-btn {
    background: none;
    border: 1px solid #252530;
    color: #6b7a8d;
    cursor: pointer;
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 3px;
    line-height: 1;
  }
  .refresh-btn:hover { color: #8bb8e8; border-color: #5b9bd5; }
  .refresh-btn:disabled { opacity: 0.4; cursor: default; }

  .panel-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }
  .panel-body::-webkit-scrollbar { width: 4px; }
  .panel-body::-webkit-scrollbar-track { background: transparent; }
  .panel-body::-webkit-scrollbar-thumb { background: #252530; border-radius: 2px; }

  /* Empty / loading / error states */
  .state-loading,
  .state-empty {
    font-size: 11px;
    color: #4a5568;
    padding: 16px 8px;
    text-align: center;
    font-style: italic;
  }

  .state-error {
    font-size: 11px;
    color: #b05050;
    padding: 12px 8px;
    text-align: center;
  }

  /* Variable row */
  .var-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    background: none;
    border: none;
    border-bottom: 1px solid #1e1e28;
    color: #b8c4d4;
    font-size: 11px;
    font-family: monospace;
    cursor: pointer;
    text-align: left;
    border-radius: 0;
    transition: background 0.1s;
  }
  .var-row:hover { background: #1e1e28; }
  .var-row.var-expanded { background: #1a1a24; border-left: 2px solid #5b9bd5; }

  .var-name {
    color: #8bb8e8;
    font-weight: 600;
    flex-shrink: 0;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .var-changes {
    flex: 1;
    font-size: 10px;
    color: #8b95a5;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .var-chevron {
    font-size: 9px;
    color: #4a5568;
    flex-shrink: 0;
    width: 10px;
    text-align: center;
  }

  /* Timeline expansion */
  .timeline-section {
    padding: 4px 8px 8px 16px;
    border-left: 2px solid #252530;
    margin-left: 8px;
    margin-bottom: 4px;
  }

  .tl-empty {
    font-size: 10px;
    color: #4a5568;
    padding: 6px 0;
    font-style: italic;
  }

  .tl-entry {
    font-family: monospace;
    font-size: 10px;
    color: #8b95a5;
    padding: 3px 0;
    border-bottom: 1px solid #1e1e28;
  }
  .tl-entry:last-child { border-bottom: none; }
</style>
