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

<div class="flex flex-col bg-dark max-h-full h-full overflow-hidden">
  <!-- Header -->
  <div class="flex items-center justify-between px-2.5 py-2 border-b border-border flex-shrink-0">
    <span class="text-[10px] text-text-muted uppercase tracking-[0.5px] font-semibold">Session Timeline</span>
    <span class="text-[10px] text-text-dim bg-hover px-1.5 py-px rounded-lg font-mono">{steps.length}</span>
  </div>

  <!-- Body -->
  <div class="timeline-body flex-1 overflow-y-auto py-1" bind:this={scrollContainer}>
    {#if steps.length === 0}
      <div class="py-5 px-3 text-text-dim text-[11px] text-center leading-relaxed">No steps yet. Use the command bar to call a function.</div>
    {:else}
      <ol class="list-none m-0 p-0">
        {#each steps as step (step.step_index)}
          {@const badge = accessBadge(step.access)}
          {@const isHighlighted = highlighted === step.function}
          {@const isExpanded = expandedStep === step.step_index}
          <li
            class="border-l-3 transition-[border-color] duration-150 {isHighlighted ? 'border-l-accent' : 'border-l-transparent'}"
          >
            <button
              class="flex items-center gap-1.5 w-full px-2.5 py-1.5 bg-transparent border-none text-text text-[11px] font-mono cursor-pointer text-left hover:bg-hover {isExpanded ? 'bg-surface-alt' : ''}"
              onclick={() => toggleNarrative(step.step_index)}
            >
              <span class="text-text-dim text-[9px] min-w-[16px] text-right">{step.step_index}</span>
              <span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{step.function}</span>
              <span class="badge {badge.cls}">{badge.text}</span>
              <span class="text-text-dim text-[9px] flex-shrink-0">{isExpanded ? '▾' : '▸'}</span>
            </button>

            {#if isExpanded}
              <div class="py-2 pr-3 pl-7 border-t border-hover bg-surface">
                {#if loadingNarrative}
                  <span class="text-text-dim text-[10px] italic">Loading...</span>
                {:else}
                  <pre class="text-text-muted text-[11px] font-mono m-0 whitespace-pre-wrap break-words leading-relaxed">{narrative}</pre>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ol>
    {/if}
  </div>

  <!-- Footer -->
  {#if steps.length > 0}
    <div class="px-2.5 py-1.5 border-t border-border flex-shrink-0">
      <button
        class="w-full py-[5px] bg-hover border border-border rounded-[4px] text-text-muted text-[11px] font-mono cursor-pointer transition-[color,border-color] duration-150 hover:text-text hover:border-accent"
        onclick={goBack}
        disabled={backBusy}
      >
        ← Back
      </button>
    </div>
  {/if}
</div>

<style>
  /* Access badges — use semantic colors from theme */
  .badge {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    font-family: system-ui, sans-serif;
    flex-shrink: 0;
    text-transform: lowercase;
  }

  .badge-public {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .badge-internal {
    color: var(--color-text-muted);
    background: color-mix(in srgb, var(--color-text-muted) 12%, transparent);
  }

  .badge-restricted {
    color: var(--color-orange);
    background: color-mix(in srgb, var(--color-orange) 12%, transparent);
  }

  .badge-special {
    color: var(--color-purple);
    background: color-mix(in srgb, var(--color-purple) 12%, transparent);
  }

  /* Scrollbar — pseudo-elements require scoped CSS */
  .timeline-body::-webkit-scrollbar { width: 4px; }
  .timeline-body::-webkit-scrollbar-track { background: transparent; }
  .timeline-body::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 2px; }
</style>
