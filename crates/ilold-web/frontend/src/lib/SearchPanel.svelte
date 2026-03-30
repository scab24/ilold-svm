<script lang="ts">
  import { searchOpen, searchContext, toggleSearch } from '$lib/stores/search';
  import { search, type SearchResult } from '$lib/api/ws';
  import { getSearchSuggestions, type SearchSuggestions } from '$lib/api/rest';
  import { goto } from '$app/navigation';
  import DraggablePanel from '$lib/DraggablePanel.svelte';

  let query = $state('');
  let results: SearchResult[] = $state([]);
  let searching = $state(false);
  let total: number | null = $state(null);
  let debounceTimer: ReturnType<typeof setTimeout>;
  let inputEl: HTMLInputElement;

  let isOpen = $state(false);
  let currentContract: string | null = $state(null);
  let suggestions: SearchSuggestions | null = $state(null);

  searchOpen.subscribe(v => {
    isOpen = v;
    if (v) setTimeout(() => inputEl?.focus(), 50);
  });
  searchContext.subscribe(async v => {
    currentContract = v;
    if (v) {
      try { suggestions = await getSearchSuggestions(v); } catch { suggestions = null; }
    } else {
      suggestions = null;
    }
  });

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
    goto(`/contract/${r.contract}/${r.function}?path=${r.path_id}`);
  }

  function fieldColor(f: string): string {
    switch (f) {
      case 'require': return '#c49a4a';
      case 'external_call': return '#b05050';
      case 'state_write': return '#5b9bd5';
      case 'event': return '#5a9a6a';
      case 'assembly': return '#8b7ec8';
      default: return '#6b7a8d';
    }
  }

  function terminalColor(t: string): string {
    return t === 'Return' ? '#5a9a6a' : t === 'Revert' ? '#b05050' : '#6b7a8d';
  }

  function categoryColor(label: string): string {
    switch (label) {
      case 'Functions': return '#8bb8e8';
      case 'State Variables': return '#5b9bd5';
      case 'Events': return '#c49a4a';
      case 'External Calls': return '#b05050';
      case 'Path Types': return '#6b7a8d';
      default: return '#6b7a8d';
    }
  }
</script>

{#if isOpen}
  <DraggablePanel
    title={currentContract ? `Search: ${currentContract}` : 'Search: all'}
    x={12} y={50} width={300}
    onclose={toggleSearch}
  >
    <div class="search-input-wrap">
      {#if query}
        <button class="back-btn" onclick={() => { query = ''; results = []; total = null; }}>←</button>
      {/if}
      <input
        bind:this={inputEl}
        type="text"
        placeholder="Search paths..."
        bind:value={query}
        oninput={onInput}
        onkeydown={(e) => { if (e.key === 'Escape') { if (query) { query = ''; results = []; total = null; } else { toggleSearch(); } } }}
      />
      {#if searching}
        <span class="status">...</span>
      {:else if total !== null}
        <span class="status">{total}</span>
      {/if}
    </div>

    <div class="panel-body">
      {#if !query && results.length === 0}
        {#if suggestions}
          {#each suggestions.categories as cat}
            {#if cat.items.length > 0}
              <div class="suggestion-group">
                <div class="suggestion-label" style="color:{categoryColor(cat.label)}">{cat.label}</div>
                <div class="suggestion-items">
                  {#each cat.items as item}
                    <button class="pill" onclick={() => { query = item; doSearch(); }}>{item}</button>
                  {/each}
                </div>
              </div>
            {/if}
          {/each}
        {:else}
          <div class="examples">
            {#each ['transfer', 'balances', 'revert', 'external', 'owner'] as ex}
              <button class="pill" onclick={() => { query = ex; doSearch(); }}>{ex}</button>
            {/each}
          </div>
        {/if}
      {/if}

      {#each results.slice(0, 100) as r}
        <button class="result" onclick={() => goToResult(r)}>
          <div class="result-top">
            <span class="func">{r.function}</span>
            <span class="pid">#{r.path_id}</span>
            <span style="color:{terminalColor(r.terminal)}">{r.terminal}</span>
          </div>
          <div class="result-matches">
            {#each r.matches.slice(0, 3) as m}
              <span class="match" style="color:{fieldColor(m.field)}">{m.value}</span>
            {/each}
          </div>
        </button>
      {/each}

      {#if total !== null && total > 100}
        <div class="more">+{total - 100} more</div>
      {/if}

      {#if total === 0}
        <div class="empty">No results for "{query}"</div>
      {/if}
    </div>
  </DraggablePanel>
{/if}

<style>
  .back-btn {
    background: none; border: none;
    color: #6b7a8d; cursor: pointer;
    font-size: 14px; padding: 4px 6px;
    border-radius: 4px; flex-shrink: 0;
  }
  .back-btn:hover { background: #252830; color: #b8c4d4; }

  .search-input-wrap {
    display: flex; align-items: center;
    padding: 6px; gap: 4px;
    border-bottom: 1px solid #2a2d38;
  }

  .search-input-wrap input {
    flex: 1;
    padding: 6px 10px;
    background: transparent;
    border: 1px solid #2a2d38;
    border-radius: 6px;
    color: #b8c4d4;
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }
  .search-input-wrap input:focus { border-color: #5b9bd5; }
  .search-input-wrap input::placeholder { color: #4a5568; }

  .status { font-size: 10px; color: #6b7a8d; padding: 0 4px; }

  .panel-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  .suggestion-group {
    padding: 6px 8px;
  }
  .suggestion-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
    font-weight: 600;
  }
  .suggestion-items {
    display: flex; flex-wrap: wrap; gap: 3px;
  }
  .examples {
    display: flex; flex-wrap: wrap; gap: 4px; padding: 8px;
  }
  .pill {
    background: #1e2028; border: 1px solid #2a2d38;
    color: #6b7a8d; padding: 3px 10px;
    border-radius: 12px; cursor: pointer;
    font-size: 11px; font-family: monospace;
  }
  .pill:hover { border-color: #5b9bd5; color: #b8c4d4; }

  .result {
    display: block; width: 100%;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    color: inherit; font: inherit;
  }
  .result:hover { background: #252830; }

  .result-top {
    display: flex; align-items: center;
    gap: 4px; font-size: 11px;
  }
  .func { color: #b8c4d4; font-weight: 600; font-family: monospace; font-size: 11px; }
  .pid { color: #4a5568; font-size: 9px; }

  .result-matches {
    display: flex; gap: 3px; margin-top: 1px; flex-wrap: wrap;
  }
  .match {
    font-size: 9px; font-family: monospace;
    background: #181a2088; padding: 1px 4px;
    border-radius: 3px;
  }

  .result-contract {
    font-size: 9px; color: #4a5568; margin-top: 1px;
  }

  .more, .empty {
    text-align: center; padding: 12px;
    font-size: 11px; color: #4a5568;
  }
</style>
