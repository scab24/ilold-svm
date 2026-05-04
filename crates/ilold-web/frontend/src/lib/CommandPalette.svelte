<script lang="ts">
  // F5 — centered Cmd+K command palette. Replaces the old draggable
  // SearchPanel. Routes publish context-specific commands via
  // `setPaletteCommands`; this component filters + renders them and also
  // streams WebSocket path-search results under a `Path` category when
  // the query is long enough, so path search stays one keystroke away.
  import { onDestroy, onMount, tick } from 'svelte';
  import {
    isPaletteOpen,
    closePalette,
    getPaletteCommands,
  } from '$lib/stores/palette.svelte';
  import {
    CATEGORY_ORDER,
    type Command,
    type CommandCategory,
  } from '$lib/commands/registry';
  import { scoreWithKeywords, matchPositions } from '$lib/utils/fuzzy';
  import { search, subscribe, getConnectionState } from '$lib/api/ws';
  import { getSearchContext, setSearchNavigate } from '$lib/stores/search.svelte';
  import { getSearchSuggestions, type SearchSuggestions } from '$lib/api/rest';
  import type { SearchResult, ConnectionEvent, ConnectionState } from '$lib/api/types';

  let query = $state('');
  let selectedIdx = $state(0);
  let inputEl: HTMLInputElement | null = $state(null);
  let listEl: HTMLDivElement | null = $state(null);

  // WebSocket path-search plumbing — keyed off the palette query but only
  // fires once the user has typed ≥ 2 chars, matching the old SearchPanel
  // threshold so we don't spam the backend on every keystroke.
  let pathResults = $state<SearchResult[]>([]);
  let pathSearching = $state(false);
  let pathTotal = $state<number | null>(null);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let cancelActiveSearch: (() => void) | null = null;

  // Fallback pills when the backend hasn't returned a suggestions payload
  // yet (or fails). Keeps the palette useful the moment it opens on a
  // contract instead of showing an empty suggestions list.
  const FALLBACK_SUGGESTIONS: readonly string[] = [
    'transfer', 'balances', 'revert', 'external', 'owner', 'approve',
  ];

  // Suggestions (identifiers in the current contract — functions, state
  // vars, events, external calls). Loaded once per palette open when a
  // contract context is active. Clicking a suggestion pre-fills the query,
  // which then streams live path results via the debounced WS search.
  let suggestions = $state<SearchSuggestions | null>(null);

  // Most-recently-used command ids — persisted in localStorage so the
  // user's "Recent" list survives reloads. Suggestions and Path rows
  // aren't worth replaying (they're searches, not actions) so we filter
  // those out before persisting. Capped at MRU_MAX entries.
  const MRU_KEY = 'ilold:palette:mru';
  const MRU_MAX = 5;
  let mruIds = $state<string[]>(loadMru());

  function loadMru(): string[] {
    if (typeof window === 'undefined') return [];
    try {
      const raw = window.localStorage.getItem(MRU_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed.filter((x) => typeof x === 'string').slice(0, MRU_MAX) : [];
    } catch {
      return [];
    }
  }

  function recordMru(cmd: Command) {
    if (cmd.category === 'Suggestion' || cmd.category === 'Path') return;
    // Recent rows are clones whose id is `recent:<original>` — strip the
    // prefix so re-running a recent collapses to the same MRU entry.
    const canonicalId = cmd.id.startsWith('recent:') ? cmd.id.slice(7) : cmd.id;
    const next = [canonicalId, ...mruIds.filter((id) => id !== canonicalId)].slice(0, MRU_MAX);
    mruIds = next;
    if (typeof window !== 'undefined') {
      try { window.localStorage.setItem(MRU_KEY, JSON.stringify(next)); } catch {}
    }
  }

  // Mirror the global WS state so the palette can warn when path search
  // is unavailable (server killed, reconnect in flight). We hydrate
  // synchronously from the existing getter and subscribe only while
  // mounted to avoid leaking on hot-reload.
  let wsState = $state<ConnectionState>(getConnectionState());
  onMount(() => {
    const unsub = subscribe('connection', (e: ConnectionEvent) => { wsState = e.state; });
    return unsub;
  });

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
    return t === 'Return'
      ? 'var(--color-success)'
      : t === 'Revert'
        ? 'var(--color-danger)'
        : 'var(--color-text-muted)';
  }

  function categoryIcon(label: string): string {
    switch (label) {
      case 'Functions': return 'ƒ';
      case 'State Variables': return '§';
      case 'Events': return '⚡';
      case 'External Calls': return '↗';
      case 'Path Types': return '⇵';
      default: return '•';
    }
  }

  // Suggestion section sub-headers — same colour mapping the old
  // SearchPanel used so auditors who'd memorised those colours don't lose
  // the visual cue.
  function suggestionHeaderColor(label: string): string {
    switch (label) {
      case 'Functions': return 'var(--color-accent-hover)';
      case 'State Variables': return 'var(--color-accent)';
      case 'Events': return 'var(--color-warning)';
      case 'External Calls': return 'var(--color-danger)';
      case 'Path Types': return 'var(--color-text-muted)';
      default: return 'var(--color-text-dim)';
    }
  }

  const paletteOpen = $derived(isPaletteOpen());
  // Safety net: Svelte's keyed each throws on duplicate ids, and upstream
  // publishers (ProjectMap with repeated interfaces, overloaded Solidity
  // functions) occasionally produce collisions. We silently keep the
  // first occurrence so the palette stays renderable even if a caller
  // slips up — the id uniqueness is their contract, this is belt-and-
  // suspenders.
  const baseCommands = $derived.by(() => {
    const raw = getPaletteCommands();
    const seen = new Set<string>();
    const out: Command[] = [];
    for (const c of raw) {
      if (seen.has(c.id)) continue;
      seen.add(c.id);
      out.push(c);
    }
    return out;
  });

  // Rank commands. Empty query preserves registration order grouped by
  // category; non-empty query scores everything and drops non-matches.
  const ranked = $derived.by(() => {
    const q = query.trim();
    if (!q) {
      return baseCommands.map((cmd, i) => ({ cmd, s: -i }));
    }
    return baseCommands
      .map((cmd) => ({ cmd, s: scoreWithKeywords(cmd.label, cmd.keywords, q) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => b.s - a.s);
  });

  // Appended live path-search results. We don't merge them into the main
  // `ranked` list because they arrive asynchronously and shouldn't bump
  // the user's current keyboard selection around.
  const pathCommands = $derived<Command[]>(
    pathResults.map((r, i) => ({
      id: `path:${r.contract}:${r.function}:${r.path_id}:${i}`,
      label: `${r.function} #${r.path_id}`,
      detail: r.contract,
      category: 'Path' as const,
      icon: r.terminal === 'Return' ? '↩' : r.terminal === 'Revert' ? '⚠' : '•',
      pathMeta: {
        contract: r.contract,
        terminal: r.terminal,
        matches: r.matches.slice(0, 4),
      },
      run: () => {
        setSearchNavigate({ contract: r.contract, func: r.function, pathId: r.path_id });
      },
    })),
  );

  // Suggestions → `Suggestion` category commands, shown ONLY when the
  // query is empty. Clicking pre-fills the query so the user can then see
  // live path matches stream in. Falls back to a curated list when the
  // backend hasn't delivered suggestions yet.
  const suggestionCommands = $derived.by<Command[]>(() => {
    if (query) return [];
    const make = (item: string, cat: string, idx: number): Command => ({
      id: `sugg:${cat}:${idx}:${item}`,
      label: item,
      detail: cat,
      category: 'Suggestion',
      icon: categoryIcon(cat),
      keepOpenOnRun: true,
      run: () => { query = item; tick().then(() => inputEl?.focus()); },
    });
    if (suggestions && suggestions.categories.some((c) => c.items.length > 0)) {
      return suggestions.categories.flatMap((cat) =>
        cat.items.map((item, i) => make(item, cat.label, i)),
      );
    }
    // Only show fallback when a contract is loaded — on the home screen
    // path search is meaningless anyway.
    if (getSearchContext()) {
      return FALLBACK_SUGGESTIONS.map((s, i) => make(s, 'Common', i));
    }
    return [];
  });

  // Recent commands — only when the query is empty (otherwise the user
  // is typing a different intent). Resolves each MRU id back to the
  // current Command so a stale id (command no longer registered after
  // navigating away) is silently dropped. Cloned with a `recent:` id
  // prefix so they have a unique key when also present in their original
  // category (and the keyed-each block stays valid).
  const recentCommands = $derived<Command[]>(
    !query
      ? mruIds
          .map((id) => baseCommands.find((c) => c.id === id))
          .filter((c): c is Command => c !== undefined)
          .map((c) => ({ ...c, id: `recent:${c.id}`, category: 'Recent' as const }))
      : [],
  );

  const visible = $derived<{ cmd: Command }[]>([
    ...recentCommands.map((cmd) => ({ cmd })),
    ...suggestionCommands.map((cmd) => ({ cmd })),
    ...ranked.map(({ cmd }) => ({ cmd })),
    ...pathCommands.map((cmd) => ({ cmd })),
  ]);

  // Group the visible rows by category while preserving the sorted order
  // inside each group. CATEGORY_ORDER dictates section ordering.
  const grouped = $derived.by(() => {
    const byCat = new Map<CommandCategory, Command[]>();
    for (const { cmd } of visible) {
      const arr = byCat.get(cmd.category) ?? [];
      arr.push(cmd);
      byCat.set(cmd.category, arr);
    }
    const out: { category: CommandCategory; cmds: Command[] }[] = [];
    for (const cat of CATEGORY_ORDER) {
      const cmds = byCat.get(cat);
      if (cmds && cmds.length > 0) out.push({ category: cat, cmds });
    }
    return out;
  });

  // Flat list used by keyboard navigation — mirrors the render order.
  const flat = $derived<Command[]>(grouped.flatMap((g) => g.cmds));

  // Reset + focus on open; tear down search on close. Also fetch the
  // per-contract suggestion list once per open — we intentionally don't
  // reuse a stale list across opens so the caller (the contract page)
  // can update its analysis and we'll pick up fresh pills.
  $effect(() => {
    if (paletteOpen) {
      query = '';
      selectedIdx = 0;
      pathResults = [];
      tick().then(() => inputEl?.focus());
      const ctx = getSearchContext();
      if (ctx) {
        getSearchSuggestions(ctx)
          .then((s) => { suggestions = s; })
          .catch(() => { suggestions = null; });
      } else {
        suggestions = null;
      }
    } else {
      teardownSearch();
    }
  });

  // Clamp selection when the result set shrinks so Enter never fires a
  // stale row.
  $effect(() => {
    const n = flat.length;
    if (selectedIdx >= n) selectedIdx = Math.max(0, n - 1);
  });

  // Debounced path search. Fires only on the contract page (where
  // getSearchContext() returns a name) and only for queries that survive
  // the 2-char threshold.
  $effect(() => {
    const q = query.trim();
    const ctx = getSearchContext();
    clearDebounce();
    if (!paletteOpen) return;
    if (q.length < 2) {
      teardownSearch();
      return;
    }
    debounceTimer = setTimeout(() => runPathSearch(q, ctx), 250);
  });

  onDestroy(() => {
    clearDebounce();
    teardownSearch();
  });

  function clearDebounce() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function teardownSearch() {
    cancelActiveSearch?.();
    cancelActiveSearch = null;
    pathResults = [];
    pathSearching = false;
    pathTotal = null;
  }

  function runPathSearch(q: string, ctx: string | null) {
    teardownSearch();
    pathSearching = true;
    cancelActiveSearch = search(
      q,
      {
        onResult: (r) => {
          // Same cap the old SearchPanel used — past 100 the +N-more hint
          // tells the user to refine instead of flooding the list.
          if (pathResults.length < 100) pathResults = [...pathResults, r];
        },
        onComplete: (total) => {
          pathSearching = false;
          pathTotal = total;
        },
        onError: () => {
          pathSearching = false;
          pathTotal = 0;
        },
      },
      ctx ? { contract: ctx } : undefined,
    );
  }

  async function onKeydown(e: KeyboardEvent) {
    if (!paletteOpen) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      // Two-step Escape (same as the old SearchPanel): first press clears
      // a non-empty query so the user can go back to the full command
      // list; second press actually closes the palette.
      if (query) {
        query = '';
        selectedIdx = 0;
      } else {
        closePalette();
      }
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (flat.length) selectedIdx = (selectedIdx + 1) % flat.length;
      scrollSelectedIntoView();
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (flat.length) selectedIdx = (selectedIdx - 1 + flat.length) % flat.length;
      scrollSelectedIntoView();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const cmd = flat[selectedIdx];
      if (cmd) {
        // Close BEFORE running so async handlers don't race with the
        // palette being re-opened by a subsequent Cmd+K. Suggestions
        // opt out with keepOpenOnRun so they can pre-fill the query.
        if (!cmd.keepOpenOnRun) closePalette();
        recordMru(cmd);
        await cmd.run();
      }
    }
  }

  async function scrollSelectedIntoView() {
    await tick();
    const row = listEl?.querySelector<HTMLElement>(`[data-idx="${selectedIdx}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }

  function runCommand(cmd: Command) {
    if (!cmd.keepOpenOnRun) closePalette();
    recordMru(cmd);
    cmd.run();
  }

  function categoryLabel(c: CommandCategory): string {
    switch (c) {
      case 'Recent': return 'Recent';
      case 'Suggestion': return 'Suggestions';
      case 'Action': return 'Actions';
      case 'Mode': return 'Modes';
      case 'Scenario': return 'Scenarios';
      case 'Function': return 'Functions';
      case 'Contract': return 'Contracts';
      case 'Path': return 'Paths';
    }
  }

  // Stable flat-index lookup so the click handlers can know whether a row
  // is the currently highlighted one.
  function flatIndexOf(cmd: Command): number {
    return flat.findIndex((c) => c.id === cmd.id);
  }

  // Group suggestion commands by their `detail` (which holds the backend
  // category label like "Functions"). Preserves first-seen order so the
  // visual ordering tracks how the backend returned them.
  function subgroupSuggestions(cmds: Command[]): { label: string; cmds: Command[] }[] {
    const map = new Map<string, Command[]>();
    for (const c of cmds) {
      const key = c.detail ?? 'Suggestions';
      const arr = map.get(key) ?? [];
      arr.push(c);
      map.set(key, arr);
    }
    return Array.from(map, ([label, cmds]) => ({ label, cmds }));
  }
</script>

<!-- Reusable row markup — extracted as a snippet so the suggestion
     sub-grouped path and the standard category path don't duplicate it. -->
{#snippet commandRow(cmd: Command)}
  {@const idx = flatIndexOf(cmd)}
  {@const positions = query ? matchPositions(cmd.label, query) : []}
  {@const posSet = new Set(positions)}
  <button
    type="button"
    class="row"
    class:selected={idx === selectedIdx}
    id={`palette-row-${idx}`}
    data-idx={idx}
    role="option"
    aria-selected={idx === selectedIdx}
    onclick={() => runCommand(cmd)}
    onmouseenter={() => (selectedIdx = idx)}
  >
    {#if cmd.icon}
      <span class="row-icon" aria-hidden="true">{cmd.icon}</span>
    {/if}
    <span class="row-label">
      {#if posSet.size > 0}
        {#each cmd.label as ch, i}<span class:hl={posSet.has(i)}>{ch}</span>{/each}
      {:else}
        {cmd.label}
      {/if}
    </span>
    {#if cmd.pathMeta}
      <span class="row-terminal" style="color:{terminalColor(cmd.pathMeta.terminal)}">{cmd.pathMeta.terminal}</span>
      <span class="row-matches">
        {#each cmd.pathMeta.matches as m}
          <span class="match-token" style="color:{fieldColor(m.field)}">{m.value}</span>
        {/each}
      </span>
      <span class="row-contract">{cmd.pathMeta.contract}</span>
    {:else if cmd.detail}
      <span class="row-detail">{cmd.detail}</span>
    {/if}
    {#if cmd.shortcut}
      <span class="row-shortcut" aria-label="Keyboard shortcut: {cmd.shortcut}">
        {#each cmd.shortcut.split('+') as part, i}
          {#if i > 0}<span class="shortcut-plus">+</span>{/if}
          <kbd>{part}</kbd>
        {/each}
      </span>
    {/if}
  </button>
{/snippet}

<svelte:window onkeydown={onKeydown} />

{#if paletteOpen}
  <!-- Backdrop: click closes. Using a button with transparent styling
       keeps the keyboard-activation story simple and a11y-clean. -->
  <div class="backdrop" role="presentation">
    <button
      type="button"
      class="backdrop-click"
      aria-label="Close command palette"
      onclick={() => closePalette()}
    ></button>

    <div
      class="palette"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div class="input-row">
        <span class="input-icon" aria-hidden="true">⌘</span>
        <input
          bind:this={inputEl}
          bind:value={query}
          type="text"
          class="input"
          placeholder={getSearchContext()
            ? `Search ${getSearchContext()} or run a command…`
            : 'Type a command, function or contract…'}
          aria-label="Command palette query"
          aria-controls="palette-list"
          aria-activedescendant={flat[selectedIdx] ? `palette-row-${flatIndexOf(flat[selectedIdx])}` : undefined}
          autocomplete="off"
          spellcheck="false"
        />
        {#if pathSearching}
          <span class="input-status" aria-label="Searching">…</span>
        {:else if pathTotal !== null && query.length >= 2}
          <span class="input-status" aria-label="{pathTotal} path results">{pathTotal}</span>
        {/if}
        {#if query}
          <button
            type="button"
            class="input-clear"
            onclick={() => { query = ''; inputEl?.focus(); }}
            aria-label="Clear query"
            title="Clear"
          >✕</button>
        {/if}
      </div>

      <div
        bind:this={listEl}
        id="palette-list"
        class="list"
        role="listbox"
      >
        {#if flat.length === 0}
          <div class="empty">
            {#if query && query.length >= 2 && pathTotal === 0 && !pathSearching}
              No results for "{query}"
            {:else if query}
              No matches for "{query}"
            {:else}
              No commands registered
            {/if}
          </div>
        {:else}
          {#each grouped as group (group.category)}
            {#if group.category === 'Suggestion'}
              <!-- Suggestions get sub-headers per backend category
                   (Functions / State Variables / Events / External Calls /
                   Path Types) with the colour mapping from the old
                   SearchPanel — the parent "Suggestions" header is
                   suppressed because the sub-headers carry the meaning. -->
              {#each subgroupSuggestions(group.cmds) as sub (sub.label)}
                <div
                  class="group-label sub"
                  style="color:{suggestionHeaderColor(sub.label)}"
                >{sub.label}</div>
                {#each sub.cmds as cmd (cmd.id)}
                  {@render commandRow(cmd)}
                {/each}
              {/each}
            {:else}
              <div class="group-label">{categoryLabel(group.category)}</div>
              {#each group.cmds as cmd (cmd.id)}
                {@render commandRow(cmd)}
              {/each}
            {/if}
          {/each}
          {#if query.length >= 2 && pathTotal !== null && pathTotal > pathResults.length}
            {@const extra = pathTotal - pathResults.length}
            <div class="more-hint">+{extra} more path result{extra === 1 ? '' : 's'} — refine to see them</div>
          {/if}
        {/if}
      </div>

      <div class="footer">
        <span aria-hidden="true"><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
        <span aria-hidden="true"><kbd>↵</kbd> select</span>
        <span aria-hidden="true"><kbd>esc</kbd> close</span>
        {#if wsState !== 'connected'}
          <span class="ws-offline" role="status" aria-live="polite">
            ● {wsState === 'connecting' ? 'WS reconnecting…' : 'WS offline — path search disabled'}
          </span>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    /* clamp the top padding so it scales with viewport height but
       collapses on phone-landscape / very short panes. */
    padding-top: clamp(24px, 12vh, 110px);
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
  }
  .backdrop-click {
    position: absolute;
    inset: 0;
    background: transparent;
    border: none;
    cursor: default;
    padding: 0;
  }
  .palette {
    position: relative;
    z-index: 1;
    /* clamp(min, preferred, max) — narrow viewports get most of the
       width minus a small inset, wide viewports cap at 560px. */
    width: clamp(280px, 90vw, 560px);
    /* Bound by both viewport and an absolute ceiling so on a 13" laptop
       in landscape (~700px tall) the palette doesn't bleed below the
       fold. The list inside scrolls. */
    max-height: min(70vh, 520px);
    display: flex;
    flex-direction: column;
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.96) 0%, rgba(20, 20, 28, 0.98) 100%);
    border: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
    border-radius: 12px;
    box-shadow:
      0 25px 60px -15px rgba(0, 0, 0, 0.55),
      0 12px 30px -10px rgba(0, 0, 0, 0.35),
      0 0 0 1px rgba(91, 155, 213, 0.08),
      0 0 80px -20px rgba(91, 155, 213, 0.12);
    overflow: hidden;
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid color-mix(in srgb, var(--color-border) 40%, transparent);
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
  }
  .input-icon {
    color: var(--color-text-dim);
    font-size: 14px;
    line-height: 1;
  }
  .input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--color-text);
    font-family: var(--font-mono, monospace);
    font-size: 13px;
  }
  .input::placeholder { color: var(--color-text-dim); }
  .input-status {
    color: var(--color-text-dim);
    font-size: 10px;
    font-family: var(--font-mono, monospace);
    min-width: 20px;
    text-align: right;
  }
  .input-clear {
    background: transparent;
    border: none;
    color: var(--color-text-dim);
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 4px;
    font-size: 11px;
    line-height: 1;
    transition: color 100ms ease, background 100ms ease;
  }
  .input-clear:hover {
    color: var(--color-text);
    background: color-mix(in srgb, var(--color-border) 40%, transparent);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .group-label {
    padding: 8px 14px 4px;
    font-size: 9px;
    font-weight: 700;
    color: var(--color-text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    user-select: none;
  }
  /* Suggestion sub-headers are styled inline with category-specific
     colours; this base only sets the spacing so they sit slightly
     indented under the implicit Suggestions section. */
  .group-label.sub { padding-top: 6px; }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 14px;
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease;
  }
  .row.selected {
    background: color-mix(in srgb, var(--color-accent) 14%, transparent);
    color: var(--color-accent-light);
  }
  .row-icon {
    width: 16px;
    text-align: center;
    color: var(--color-text-dim);
    font-size: 12px;
    flex-shrink: 0;
  }
  .row.selected .row-icon { color: var(--color-accent-hover); }
  .row-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Per-character highlight for fuzzy-matched chars. The accent colour
     pops against both selected and idle rows; bold weight reinforces
     the cue without changing layout. */
  .row-label .hl {
    color: var(--color-accent-hover);
    font-weight: 700;
  }
  .row.selected .row-label .hl {
    color: var(--color-accent-light);
  }
  .row-detail {
    font-size: 10px;
    color: var(--color-text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50%;
  }
  .row.selected .row-detail { color: var(--color-text-muted); }

  /* Rich rendering for path-search rows (pathMeta present). Colours the
     terminal pill, each match token by field kind, and keeps the origin
     contract on the far right. */
  .row-terminal {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: 3px;
    background: color-mix(in srgb, currentColor 12%, transparent);
    flex-shrink: 0;
  }
  .row-matches {
    display: inline-flex;
    gap: 4px;
    overflow: hidden;
    flex: 1;
    min-width: 0;
  }
  .match-token {
    font-size: 9px;
    font-family: var(--font-mono, monospace);
    padding: 1px 5px;
    border-radius: 3px;
    background: color-mix(in srgb, currentColor 10%, transparent);
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-contract {
    font-size: 9px;
    color: var(--color-text-dim);
    white-space: nowrap;
    flex-shrink: 0;
    margin-left: auto;
  }
  .row.selected .row-contract { color: var(--color-text-muted); }

  /* Right-edge shortcut chip — same kbd styling as the footer hints so
     the visual language stays consistent. */
  .row-shortcut {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }
  .row-shortcut kbd {
    font-family: var(--font-mono, monospace);
    font-size: 9px;
    color: var(--color-text-muted);
    background: color-mix(in srgb, var(--color-border) 35%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-border) 55%, transparent);
    border-radius: 3px;
    padding: 0 4px;
    line-height: 1.4;
  }
  .row.selected .row-shortcut kbd {
    color: var(--color-accent-light);
    background: color-mix(in srgb, var(--color-accent) 18%, transparent);
    border-color: color-mix(in srgb, var(--color-accent) 40%, transparent);
  }
  .shortcut-plus {
    font-size: 9px;
    color: var(--color-text-dim);
  }

  .empty {
    padding: 24px 16px;
    text-align: center;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
  }
  .more-hint {
    padding: 8px 14px;
    font-size: 10px;
    color: var(--color-text-dim);
    text-align: center;
    font-style: italic;
    border-top: 1px dashed color-mix(in srgb, var(--color-border) 30%, transparent);
    margin-top: 6px;
  }

  .footer {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    border-top: 1px solid color-mix(in srgb, var(--color-border) 30%, transparent);
    font-size: 9px;
    color: var(--color-text-dim);
    background: rgba(16, 16, 22, 0.6);
    user-select: none;
  }
  .footer kbd {
    font-family: var(--font-mono, monospace);
    font-size: 9px;
    color: var(--color-text-muted);
    background: color-mix(in srgb, var(--color-border) 35%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-border) 55%, transparent);
    border-radius: 3px;
    padding: 0 4px;
    margin: 0 2px;
  }
  .ws-offline {
    margin-left: auto;
    color: var(--color-warning);
    font-size: 9px;
    letter-spacing: 0.05em;
  }
</style>
