<script lang="ts">
  import { getSteps, getHighlightedFunction } from '$lib/stores/session.svelte';
  import { getStepNarrative, postCommand } from '$lib/api/session';
  import { formatAccess } from '$lib/utils/access';
  import type { AccessLevel } from '$lib/api/types';

  interface Props {
    contract: string;
  }

  let { contract }: Props = $props();

  let expandedStep = $state<number | null>(null);
  let narrative = $state<string>('');
  let loadingNarrative = $state(false);
  let backBusy = $state(false);
  let narrativeGen = 0;
  let scrollContainer: HTMLDivElement | undefined = $state();

  const steps = $derived(getSteps());
  const highlighted = $derived(getHighlightedFunction());

  // Auto-scroll when new steps arrive
  let prevLength = 0;
  $effect(() => {
    const len = steps.length;
    if (len > prevLength && scrollContainer) {
      requestAnimationFrame(() => {
        scrollContainer!.scrollTop = scrollContainer!.scrollHeight;
      });
    }
    prevLength = len;
  });

  function accessBadge(access: AccessLevel): { text: string; cls: string } {
    const label = formatAccess(access);
    if (label === 'Public') return { text: label, cls: 'badge-public' };
    if (label === 'Internal') return { text: label, cls: 'badge-internal' };
    if (label.startsWith('Restricted')) return { text: label, cls: 'badge-restricted' };
    if (label.startsWith('Special')) return { text: label, cls: 'badge-special' };
    return { text: label, cls: 'badge-internal' };
  }

  async function toggleNarrative(stepIndex: number) {
    if (expandedStep === stepIndex) {
      expandedStep = null;
      narrative = '';
      return;
    }
    expandedStep = stepIndex;
    narrative = '';
    loadingNarrative = true;
    const gen = ++narrativeGen;
    try {
      const res = await getStepNarrative(stepIndex);
      if (gen !== narrativeGen) return; // stale
      narrative = typeof res === 'string' ? res : (res.narrative ?? JSON.stringify(res, null, 2));
    } catch (e: unknown) {
      if (gen !== narrativeGen) return;
      narrative = `Error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      if (gen === narrativeGen) loadingNarrative = false;
    }
  }

  async function goBack() {
    if (backBusy) return;
    backBusy = true;
    try {
      await postCommand('Back', contract);
    } catch (e) {
      console.warn('Back failed:', e);
    } finally {
      backBusy = false;
    }
  }
</script>

<div class="timeline-root">
  <div class="timeline-header">
    <span class="timeline-title">Session Timeline</span>
    <span class="step-count">{steps.length}</span>
  </div>

  <div class="timeline-body" bind:this={scrollContainer}>
    {#if steps.length === 0}
      <div class="empty-state">No steps yet. Use the command bar to call a function.</div>
    {:else}
      <ol class="step-list">
        {#each steps as step (step.step_index)}
          {@const badge = accessBadge(step.access)}
          {@const isHighlighted = highlighted === step.function}
          {@const isExpanded = expandedStep === step.step_index}
          <li
            class="step-item"
            class:highlighted={isHighlighted}
            class:expanded={isExpanded}
          >
            <button class="step-btn" onclick={() => toggleNarrative(step.step_index)}>
              <span class="step-index">{step.step_index}</span>
              <span class="step-fn">{step.function}</span>
              <span class="badge {badge.cls}">{badge.text}</span>
              <span class="step-chevron">{isExpanded ? '▾' : '▸'}</span>
            </button>

            {#if isExpanded}
              <div class="narrative-panel">
                {#if loadingNarrative}
                  <span class="narrative-loading">Loading...</span>
                {:else}
                  <pre class="narrative-text">{narrative}</pre>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ol>
    {/if}
  </div>

  {#if steps.length > 0}
    <div class="timeline-footer">
      <button class="back-btn" onclick={goBack} disabled={backBusy}>← Back</button>
    </div>
  {/if}
</div>

<style>
  .timeline-root {
    display: flex;
    flex-direction: column;
    background: #121215;
    max-height: 100%;
    height: 100%;
    overflow: hidden;
  }

  .timeline-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-bottom: 1px solid #252530;
    flex-shrink: 0;
  }

  .timeline-title {
    font-size: 10px;
    color: #6b7a8d;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 600;
  }

  .step-count {
    font-size: 10px;
    color: #4a5568;
    background: #1e1e28;
    padding: 1px 6px;
    border-radius: 8px;
    font-family: monospace;
  }

  .timeline-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .empty-state {
    padding: 20px 12px;
    color: #4a5568;
    font-size: 11px;
    text-align: center;
    line-height: 1.5;
  }

  .step-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .step-item {
    border-left: 3px solid transparent;
    transition: border-color 0.15s;
  }

  .step-item.highlighted {
    border-left-color: #5b9bd5;
  }

  .step-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
    background: none;
    border: none;
    color: #b8c4d4;
    font-size: 11px;
    font-family: monospace;
    cursor: pointer;
    text-align: left;
  }

  .step-btn:hover {
    background: #1e1e28;
  }

  .step-item.expanded .step-btn {
    background: #1a1a24;
  }

  .step-index {
    color: #4a5568;
    font-size: 9px;
    min-width: 16px;
    text-align: right;
  }

  .step-fn {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step-chevron {
    color: #4a5568;
    font-size: 9px;
    flex-shrink: 0;
  }

  /* Access badges */
  .badge {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    font-family: system-ui, sans-serif;
    flex-shrink: 0;
    text-transform: lowercase;
  }

  .badge-public {
    color: #5b9bd5;
    background: rgba(91, 155, 213, 0.12);
  }

  .badge-internal {
    color: #6b7a8d;
    background: rgba(107, 122, 141, 0.12);
  }

  .badge-restricted {
    color: #d4956b;
    background: rgba(212, 149, 107, 0.12);
  }

  .badge-special {
    color: #b085d6;
    background: rgba(176, 133, 214, 0.12);
  }

  /* Narrative panel */
  .narrative-panel {
    padding: 8px 12px 8px 28px;
    border-top: 1px solid #1e1e28;
    background: #18181e;
  }

  .narrative-loading {
    color: #4a5568;
    font-size: 10px;
    font-style: italic;
  }

  .narrative-text {
    color: #8a96a6;
    font-size: 11px;
    font-family: monospace;
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.5;
  }

  /* Footer with Back */
  .timeline-footer {
    padding: 6px 10px;
    border-top: 1px solid #252530;
    flex-shrink: 0;
  }

  .back-btn {
    width: 100%;
    padding: 5px 0;
    background: #1e1e28;
    border: 1px solid #252530;
    border-radius: 4px;
    color: #6b7a8d;
    font-size: 11px;
    font-family: monospace;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }

  .back-btn:hover {
    color: #b8c4d4;
    border-color: #5b9bd5;
  }

  /* Scrollbar styling */
  .timeline-body::-webkit-scrollbar {
    width: 4px;
  }

  .timeline-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .timeline-body::-webkit-scrollbar-thumb {
    background: #252530;
    border-radius: 2px;
  }
</style>
