<script lang="ts">
  import { searchOpen, toggleSearch } from '$lib/stores/search';
  import { search, type SearchResult } from '$lib/api/ws';
  import { goto } from '$app/navigation';

  let query = $state('');
  let results: SearchResult[] = $state([]);
  let searching = $state(false);
  let total: number | null = $state(null);
  let debounceTimer: ReturnType<typeof setTimeout>;
  let inputEl: HTMLInputElement;

  let isOpen = $state(false);
  searchOpen.subscribe(v => {
    isOpen = v;
    if (v) setTimeout(() => inputEl?.focus(), 50);
    if (!v) { query = ''; results = []; total = null; }
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
    });
  }

  function selectResult(r: SearchResult) {
    toggleSearch();
    goto(`/contract/${r.contract}/${r.function}`);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') toggleSearch();
  }

  function fieldColor(f: string): string {
    switch (f) {
      case 'require': return '#d29922';
      case 'external_call': return '#f85149';
      case 'state_write': return '#58a6ff';
      case 'event': return '#3fb950';
      case 'assembly': return '#8b5cf6';
      default: return '#8b949e';
    }
  }

  function terminalColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : '#8b949e';
  }

  const examples = ['transfer', 'balances', 'revert', 'external', 'owner', 'paused', 'assembly', 'deposit'];
</script>

{#if isOpen}
  <div class="overlay" onclick={toggleSearch} onkeydown={onKeydown} role="dialog" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="search-header">
        <input
          bind:this={inputEl}
          type="text"
          placeholder="Search paths... (require, transfer, balances, revert)"
          bind:value={query}
          oninput={onInput}
          onkeydown={onKeydown}
        />
        {#if searching}
          <span class="status">...</span>
        {:else if total !== null}
          <span class="status">{total}</span>
        {/if}
      </div>

      <div class="results">
        {#if !query && results.length === 0}
          <div class="examples">
            {#each examples as ex}
              <button class="example" onclick={() => { query = ex; doSearch(); }}>{ex}</button>
            {/each}
          </div>
        {/if}

        {#each results.slice(0, 50) as r}
          <button class="result" onclick={() => selectResult(r)}>
            <div class="result-top">
              <span class="contract">{r.contract}</span>
              <span class="sep">→</span>
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

        {#if total !== null && total > 50}
          <div class="more">+{total - 50} more results</div>
        {/if}

        {#if total === 0}
          <div class="no-results">No results for "{query}"</div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0;
    background: #00000088;
    z-index: 100;
    display: flex; justify-content: center;
    padding-top: 80px;
  }

  .modal {
    width: 600px; max-height: 500px;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 12px;
    box-shadow: 0 16px 48px #000000aa;
    display: flex; flex-direction: column;
    overflow: hidden;
  }

  .search-header {
    display: flex; align-items: center;
    padding: 4px;
    border-bottom: 1px solid #21262d;
  }

  .search-header input {
    flex: 1;
    padding: 12px 16px;
    background: transparent;
    border: none;
    color: #f0f6fc;
    font-size: 15px;
    font-family: inherit;
    outline: none;
  }
  .search-header input::placeholder { color: #484f58; }

  .status {
    padding: 0 12px;
    font-size: 12px;
    color: #8b949e;
  }

  .results {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  .examples {
    display: flex; flex-wrap: wrap; gap: 6px;
    padding: 12px;
  }

  .example {
    background: #21262d; border: 1px solid #30363d;
    color: #8b949e; padding: 4px 12px;
    border-radius: 16px; cursor: pointer;
    font-size: 12px; font-family: monospace;
  }
  .example:hover { border-color: #58a6ff; color: #c9d1d9; }

  .result {
    display: block; width: 100%;
    padding: 8px 12px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    color: inherit; font: inherit;
  }
  .result:hover { background: #21262d; }

  .result-top {
    display: flex; align-items: center;
    gap: 5px; font-size: 13px;
  }
  .contract { color: #8b949e; }
  .sep { color: #484f58; }
  .func { color: #f0f6fc; font-weight: 600; font-family: monospace; }
  .pid { color: #484f58; font-size: 11px; }

  .result-matches {
    display: flex; gap: 6px; margin-top: 3px;
    flex-wrap: wrap;
  }
  .match {
    font-size: 11px; font-family: monospace;
    background: #0d1117; padding: 1px 6px;
    border-radius: 3px;
  }

  .more, .no-results {
    text-align: center; padding: 12px;
    font-size: 12px; color: #484f58;
  }
</style>
