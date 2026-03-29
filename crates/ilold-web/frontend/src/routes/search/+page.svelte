<script lang="ts">
  import { search, type SearchResult } from '$lib/api/ws';

  let query = $state('');
  let results: SearchResult[] = $state([]);
  let searching = $state(false);
  let total: number | null = $state(null);
  let debounceTimer: ReturnType<typeof setTimeout>;

  function onInput() {
    clearTimeout(debounceTimer);
    if (query.trim().length < 2) {
      results = [];
      total = null;
      return;
    }
    debounceTimer = setTimeout(() => doSearch(), 300);
  }

  function doSearch() {
    results = [];
    total = null;
    searching = true;

    search(query.trim(), {
      onResult: (r) => { results = [...results, r]; },
      onComplete: (t) => { total = t; searching = false; },
      onError: (msg) => { console.error(msg); searching = false; },
    });
  }

  function fieldColor(field: string): string {
    switch (field) {
      case 'require': return '#d29922';
      case 'external_call': return '#f85149';
      case 'internal_call': return '#8b949e';
      case 'state_write': return '#58a6ff';
      case 'event': return '#3fb950';
      case 'assembly': return '#8b5cf6';
      default: return '#8b949e';
    }
  }

  function terminalColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : '#8b949e';
  }
</script>

<div class="search-view">
  <div class="topbar">
    <a href="/">← Contracts</a>
    <span class="title">Search Paths</span>
  </div>

  <div class="search-area">
    <input
      type="text"
      placeholder="Search across all paths... (e.g. transfer, balances, revert, assembly)"
      bind:value={query}
      oninput={onInput}
      class="search-input"
    />
    {#if searching}
      <span class="status">Searching...</span>
    {:else if total !== null}
      <span class="status">{total} result{total !== 1 ? 's' : ''}</span>
    {/if}
  </div>

  <div class="results">
    {#each results as r}
      <a href="/contract/{r.contract}/{r.function}" class="result-card">
        <div class="result-header">
          <span class="contract">{r.contract}</span>
          <span class="sep">→</span>
          <span class="function">{r.function}</span>
          <span class="path-id">path #{r.path_id}</span>
          <span class="terminal" style="color:{terminalColor(r.terminal)}">{r.terminal}</span>
          <span class="depth">{r.depth} blocks</span>
        </div>
        <div class="matches">
          {#each r.matches as m}
            <span class="match" style="color:{fieldColor(m.field)}">
              <span class="match-field">{m.field}:</span> {m.value}
            </span>
          {/each}
        </div>
      </a>
    {/each}

    {#if total === 0}
      <div class="no-results">No paths match "{query}"</div>
    {/if}

    {#if !query && results.length === 0}
      <div class="examples">
        <h3>Try searching for:</h3>
        <div class="example-grid">
          <button class="example" onclick={() => { query = 'transfer'; doSearch(); }}>transfer</button>
          <button class="example" onclick={() => { query = 'balances'; doSearch(); }}>balances</button>
          <button class="example" onclick={() => { query = 'revert'; doSearch(); }}>revert</button>
          <button class="example" onclick={() => { query = 'require'; doSearch(); }}>require</button>
          <button class="example" onclick={() => { query = 'external'; doSearch(); }}>external call</button>
          <button class="example" onclick={() => { query = 'assembly'; doSearch(); }}>assembly</button>
          <button class="example" onclick={() => { query = 'owner'; doSearch(); }}>owner</button>
          <button class="example" onclick={() => { query = 'paused'; doSearch(); }}>paused</button>
          <button class="example" onclick={() => { query = 'Staked'; doSearch(); }}>Staked event</button>
          <button class="example" onclick={() => { query = 'deposit'; doSearch(); }}>deposit</button>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .search-view {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    display: flex; flex-direction: column;
    background: #0d1117;
  }

  .topbar {
    display: flex; align-items: center; gap: 12px;
    padding: 8px 16px;
    background: #161b22; border-bottom: 1px solid #30363d;
    flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .title { font-size: 16px; font-weight: 700; color: #f0f6fc; }

  .search-area {
    padding: 16px 24px;
    display: flex; align-items: center; gap: 12px;
    border-bottom: 1px solid #21262d;
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    padding: 10px 16px;
    background: #0d1117;
    border: 1px solid #30363d;
    border-radius: 8px;
    color: #c9d1d9;
    font-size: 14px;
    font-family: inherit;
    outline: none;
  }
  .search-input:focus { border-color: #58a6ff; }
  .search-input::placeholder { color: #484f58; }

  .status { font-size: 13px; color: #8b949e; white-space: nowrap; }

  .results {
    flex: 1;
    overflow-y: auto;
    padding: 12px 24px;
  }

  .result-card {
    display: block;
    padding: 10px 14px;
    background: #161b22;
    border: 1px solid #21262d;
    border-radius: 6px;
    margin-bottom: 6px;
    color: inherit;
    transition: border-color 0.15s;
  }
  .result-card:hover { border-color: #58a6ff; text-decoration: none; }

  .result-header {
    display: flex; align-items: center; gap: 6px;
    font-size: 13px; margin-bottom: 4px;
  }
  .contract { color: #8b949e; }
  .sep { color: #484f58; }
  .function { color: #f0f6fc; font-weight: 600; font-family: monospace; }
  .path-id { color: #484f58; }
  .terminal { font-weight: 600; font-size: 12px; }
  .depth { color: #484f58; font-size: 11px; margin-left: auto; }

  .matches { display: flex; flex-wrap: wrap; gap: 4px; }
  .match {
    font-size: 11px; font-family: monospace;
    padding: 2px 6px;
    background: #0d1117;
    border-radius: 4px;
  }
  .match-field { opacity: 0.6; }

  .no-results {
    text-align: center; color: #484f58;
    padding: 40px; font-size: 14px;
  }
</style>
