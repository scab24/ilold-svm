<script lang="ts">
  import { onDestroy } from 'svelte';
  import { getSearchOpen, getSearchContext, setSearchNavigate, toggleSearch } from '$lib/stores/search.svelte';
  import { search } from '$lib/api/ws';
  import type { SearchResult } from '$lib/api/types';
  import { getSearchSuggestions, type SearchSuggestions } from '$lib/api/rest';
  import DraggablePanel from '$lib/DraggablePanel.svelte';

  let query = $state('');
  let results: SearchResult[] = $state([]);
  let searching = $state(false);
  let total: number | null = $state(null);
  let debounceTimer: ReturnType<typeof setTimeout>;
  let inputEl: HTMLInputElement;

  let isOpen = $derived(getSearchOpen());
  let currentContract: string | null = $state(null);
  let suggestions: SearchSuggestions | null = $state(null);

  $effect(() => {
    if (isOpen) setTimeout(() => inputEl?.focus(), 50);
  });

  $effect(() => {
    const ctx = getSearchContext();
    currentContract = ctx;
    if (!ctx) { suggestions = null; return; }
    let stale = false;
    getSearchSuggestions(ctx)
      .then(s => { if (!stale) suggestions = s; })
      .catch(() => { if (!stale) suggestions = null; });
    return () => { stale = true; };
  });

  onDestroy(() => { clearTimeout(debounceTimer); });

  function onInput() {
    clearTimeout(debounceTimer);
    if (query.trim().length < 2) {
      results = []; total = null; return;
    }
    debounceTimer = setTimeout(() => doSearch(), 300);
  }

  function doSearch() {
    results = []; total = null; searching = true;
    search(query.trim(), {
      onResult: (r) => { results = [...results, r]; },
      onComplete: (t) => { total = t; searching = false; },
      onError: () => { searching = false; },
    }, currentContract ? { contract: currentContract } : undefined);
  }

  function goToResult(r: SearchResult) {
    setSearchNavigate({ contract: r.contract, func: r.function, pathId: r.path_id });
  }

  function fieldColor(f: string): string {
    switch (f) {
      case 'require': return 'var(--color-warning)';
      case 'external_call': return 'var(--color-danger)';
      case 'state_write': return 'var(--color-accent)';
      case 'event': return 'var(--color-success)';
      case 'assembly': return 'var(--color-purple)';
      default: return 'var(--color-text-muted)';
    }
  }

  function terminalColor(t: string): string {
    return t === 'Return' ? 'var(--color-success)' : t === 'Revert' ? 'var(--color-danger)' : 'var(--color-text-muted)';
  }

  function categoryColor(label: string): string {
    switch (label) {
      case 'Functions': return 'var(--color-accent-hover)';
      case 'State Variables': return 'var(--color-accent)';
      case 'Events': return 'var(--color-warning)';
      case 'External Calls': return 'var(--color-danger)';
      case 'Path Types': return 'var(--color-text-muted)';
      default: return 'var(--color-text-muted)';
    }
  }
</script>

{#if isOpen}
  <DraggablePanel
    title={currentContract ? `Search: ${currentContract}` : 'Search: all'}
    x={12} y={50} width={300}
    onclose={toggleSearch}
  >
    <div class="flex items-center p-1.5 gap-1 border-b border-border-subtle">
      {#if query}
        <button class="bg-transparent border-none text-text-muted cursor-pointer text-sm py-1 px-1.5 rounded-sm shrink-0 hover:bg-hover hover:text-text" onclick={() => { query = ''; results = []; total = null; }}>←</button>
      {/if}
      <input
        class="search-input flex-1 py-1.5 px-2.5 bg-transparent border border-border-subtle rounded-md text-text text-xs font-[inherit] outline-none focus:border-accent placeholder:text-text-dim"
        bind:this={inputEl}
        type="text"
        placeholder="Search paths..."
        bind:value={query}
        oninput={onInput}
        onkeydown={(e) => { if (e.key === 'Escape') { if (query) { query = ''; results = []; total = null; } else { toggleSearch(); } } }}
      />
      {#if searching}
        <span class="text-[10px] text-text-muted px-1">...</span>
      {:else if total !== null}
        <span class="text-[10px] text-text-muted px-1">{total}</span>
      {/if}
    </div>

    <div class="flex-1 overflow-y-auto p-1">
      {#if !query && results.length === 0}
        {#if suggestions}
          {#each suggestions.categories as cat}
            {#if cat.items.length > 0}
              <div class="py-1.5 px-2">
                <div class="text-[9px] uppercase tracking-wide mb-1 font-semibold" style="color:{categoryColor(cat.label)}">{cat.label}</div>
                <div class="flex flex-wrap gap-0.5">
                  {#each cat.items as item}
                    <button class="pill" onclick={() => { query = item; doSearch(); }}>{item}</button>
                  {/each}
                </div>
              </div>
            {/if}
          {/each}
        {:else}
          <div class="flex flex-wrap gap-1 p-2">
            {#each ['transfer', 'balances', 'revert', 'external', 'owner'] as ex}
              <button class="pill" onclick={() => { query = ex; doSearch(); }}>{ex}</button>
            {/each}
          </div>
        {/if}
      {/if}

      {#each results.slice(0, 100) as r}
        <button class="block w-full py-1 px-2 bg-transparent border-none rounded-sm cursor-pointer text-left text-[inherit] font-[inherit] hover:bg-hover" onclick={() => goToResult(r)}>
          <div class="flex items-center gap-1 text-[11px]">
            <span class="text-text font-semibold font-mono text-[11px]">{r.function}</span>
            <span class="text-text-dim text-[9px]">#{r.path_id}</span>
            <span style="color:{terminalColor(r.terminal)}">{r.terminal}</span>
          </div>
          <div class="flex gap-0.5 mt-px flex-wrap">
            {#each r.matches.slice(0, 3) as m}
              <span class="match" style="color:{fieldColor(m.field)}">{m.value}</span>
            {/each}
          </div>
        </button>
      {/each}

      {#if total !== null && total > 100}
        <div class="text-center p-3 text-[11px] text-text-dim">+{total - 100} more</div>
      {/if}

      {#if total === 0}
        <div class="text-center p-3 text-[11px] text-text-dim">No results for "{query}"</div>
      {/if}
    </div>
  </DraggablePanel>
{/if}

<style>
  .pill {
    background: var(--color-hover); border: 1px solid var(--color-border-subtle);
    color: var(--color-text-muted); padding: 3px 10px;
    border-radius: 12px; cursor: pointer;
    font-size: 11px; font-family: monospace;
  }
  .pill:hover { border-color: var(--color-accent); color: var(--color-text); }

  .match {
    font-size: 9px; font-family: monospace;
    background: color-mix(in srgb, var(--color-surface) 50%, transparent);
    padding: 1px 4px; border-radius: 3px;
  }
</style>
