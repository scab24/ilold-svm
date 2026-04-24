<script lang="ts">
  // F5 — centered Cmd+K command palette. Replaces the old draggable
  // SearchPanel. Routes publish context-specific commands via
  // `setPaletteCommands`; this component filters + renders them and also
  // streams WebSocket path-search results under a `Path` category when
  // the query is long enough, so path search stays one keystroke away.
  import { onDestroy, tick } from 'svelte';
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
  import { scoreWithKeywords } from '$lib/utils/fuzzy';
  import { search } from '$lib/api/ws';
  import { getSearchContext, setSearchNavigate } from '$lib/stores/search.svelte';
  import type { SearchResult } from '$lib/api/types';

  let query = $state('');
  let selectedIdx = $state(0);
  let inputEl: HTMLInputElement | null = $state(null);
  let listEl: HTMLDivElement | null = $state(null);

  // WebSocket path-search plumbing — keyed off the palette query but only
  // fires once the user has typed ≥ 2 chars, matching the old SearchPanel
  // threshold so we don't spam the backend on every keystroke.
  let pathResults = $state<SearchResult[]>([]);
  let pathSearching = $state(false);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let cancelActiveSearch: (() => void) | null = null;

  const paletteOpen = $derived(isPaletteOpen());
  const baseCommands = $derived(getPaletteCommands());

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
      detail: `${r.contract} — ${r.terminal} · ${r.matches.slice(0, 2).map((m) => m.value).join(' · ')}`,
      category: 'Path' as const,
      icon: r.terminal === 'Return' ? '↩' : r.terminal === 'Revert' ? '⚠' : '•',
      run: () => {
        setSearchNavigate({ contract: r.contract, func: r.function, pathId: r.path_id });
        closePalette();
      },
    })),
  );

  const visible = $derived<{ cmd: Command }[]>([
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

  // Reset + focus on open; tear down search on close.
  $effect(() => {
    if (paletteOpen) {
      query = '';
      selectedIdx = 0;
      pathResults = [];
      tick().then(() => inputEl?.focus());
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
  }

  function runPathSearch(q: string, ctx: string | null) {
    teardownSearch();
    pathSearching = true;
    cancelActiveSearch = search(
      q,
      {
        onResult: (r) => {
          // Append but cap so a runaway query can't push the list off-screen.
          if (pathResults.length < 50) pathResults = [...pathResults, r];
        },
        onComplete: () => { pathSearching = false; },
        onError: () => { pathSearching = false; },
      },
      ctx ? { contract: ctx } : undefined,
    );
  }

  async function onKeydown(e: KeyboardEvent) {
    if (!paletteOpen) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      closePalette();
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
        // palette being re-opened by a subsequent Cmd+K.
        closePalette();
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
    closePalette();
    cmd.run();
  }

  function categoryLabel(c: CommandCategory): string {
    switch (c) {
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
</script>

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
          placeholder="Type a command, function, contract or search paths…"
          aria-label="Command palette query"
          aria-controls="palette-list"
          aria-activedescendant={flat[selectedIdx] ? `palette-row-${flatIndexOf(flat[selectedIdx])}` : undefined}
          autocomplete="off"
          spellcheck="false"
        />
        {#if pathSearching}
          <span class="input-status" aria-label="Searching">…</span>
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
            {query ? `No matches for "${query}"` : 'No commands registered'}
          </div>
        {:else}
          {#each grouped as group (group.category)}
            <div class="group-label">{categoryLabel(group.category)}</div>
            {#each group.cmds as cmd (cmd.id)}
              {@const idx = flatIndexOf(cmd)}
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
                <span class="row-label">{cmd.label}</span>
                {#if cmd.detail}
                  <span class="row-detail">{cmd.detail}</span>
                {/if}
              </button>
            {/each}
          {/each}
        {/if}
      </div>

      <div class="footer" aria-hidden="true">
        <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
        <span><kbd>↵</kbd> select</span>
        <span><kbd>esc</kbd> close</span>
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
    padding-top: 12vh;
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
    width: min(560px, calc(100vw - 32px));
    max-height: 60vh;
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
  .row-detail {
    font-size: 10px;
    color: var(--color-text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50%;
  }
  .row.selected .row-detail { color: var(--color-text-muted); }

  .empty {
    padding: 24px 16px;
    text-align: center;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
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
</style>
