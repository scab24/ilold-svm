<script lang="ts">
  import type { ProgramDetail } from '$lib/api/rest';

  let {
    program,
    canvasInstructions,
    mode,
    onadd,
    onremove,
  }: {
    program: ProgramDetail;
    canvasInstructions: Set<string>;
    mode: 'program' | 'session';
    onadd: (ix: string) => void;
    onremove: (ix: string) => void;
  } = $props();

  let sidebarOpen = $state(true);
  let query = $state('');
  let onlyPdas = $state(false);
  let onlySigners = $state(false);

  type Row = {
    name: string;
    argsCount: number;
    accountsCount: number;
    hasPdas: boolean;
    signers: string[];
  };

  const rows = $derived<Row[]>(
    (program.instructions ?? []).map((ix) => {
      const signers = (ix.accounts ?? [])
        .filter((a) => a.signer)
        .map((a) => a.name);
      const hasPdas = (ix.accounts ?? []).some((a) => a.pda != null);
      return {
        name: ix.name,
        argsCount: (ix.args ?? []).length,
        accountsCount: (ix.accounts ?? []).length,
        hasPdas,
        signers,
      };
    }),
  );

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return rows.filter((r) => {
      if (onlyPdas && !r.hasPdas) return false;
      if (onlySigners && r.signers.length === 0) return false;
      if (q && !r.name.toLowerCase().includes(q)) return false;
      return true;
    });
  });
</script>

<aside class="sidebar bg-hover border-r border-border-subtle h-full overflow-y-auto" class:closed={!sidebarOpen}>
  <div class="px-3 py-2 border-b border-border-subtle flex items-center gap-2">
    <button class="text-xs text-text-muted" onclick={() => (sidebarOpen = !sidebarOpen)}>
      {sidebarOpen ? '◀' : '▶'}
    </button>
    {#if sidebarOpen}
      <span class="text-sm text-text font-semibold">Instructions</span>
      <span class="ml-auto text-[10px] text-text-dim">{filtered.length} / {rows.length}</span>
    {/if}
  </div>
  {#if sidebarOpen}
    <div class="px-3 py-2 border-b border-border-subtle space-y-1">
      <input
        type="text"
        bind:value={query}
        placeholder="Search instruction…"
        class="w-full bg-dark border border-border-subtle rounded px-2 py-1 text-xs text-text placeholder:text-text-dim"
      />
      <div class="flex gap-2 text-[10px] text-text-muted">
        <label class="flex items-center gap-1 cursor-pointer">
          <input type="checkbox" bind:checked={onlyPdas} class="cursor-pointer" />
          PDAs
        </label>
        <label class="flex items-center gap-1 cursor-pointer">
          <input type="checkbox" bind:checked={onlySigners} class="cursor-pointer" />
          Signers
        </label>
      </div>
    </div>
    <div class="px-2 py-1.5">
      {#each filtered as row (row.name)}
        {@const onCanvas = canvasInstructions.has(row.name)}
        <div
          class="instruction-row px-2 py-1 rounded text-xs cursor-pointer hover:bg-border flex items-center gap-1.5"
          class:on-canvas={onCanvas}
        >
          <button
            class="flex-1 text-left bg-transparent border-0 p-0 cursor-pointer text-text"
            onclick={() => onadd(row.name)}
          >
            <span class="font-mono font-semibold">{row.name}</span>
            <span class="text-[10px] text-text-dim ml-1">{row.argsCount}a · {row.accountsCount}acc</span>
            {#if row.hasPdas}
              <span class="text-[9px] px-1 ml-1 rounded bg-warning/15 text-warning">pda</span>
            {/if}
            {#if row.signers.length > 0}
              <span class="text-[9px] ml-1" title={`Signers: ${row.signers.join(', ')}`}>&#x1F511;</span>
            {/if}
          </button>
          {#if onCanvas && mode !== 'session'}
            <button
              class="text-text-muted hover:text-danger px-1 bg-transparent border-0 cursor-pointer"
              onclick={() => onremove(row.name)}
              title="Remove from canvas"
            >✕</button>
          {/if}
        </div>
      {/each}
    </div>
    {#if program.account_types.length > 0}
      <div class="px-3 py-2 border-t border-border-subtle">
        <div class="text-[10px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Account types</div>
        {#each program.account_types as a (a.name)}
          <div class="px-2 py-0.5 text-[11px] font-mono text-text">{a.name}</div>
        {/each}
      </div>
    {/if}
  {/if}
</aside>

<style>
  .sidebar {
    width: 260px;
    flex-shrink: 0;
  }
  .sidebar.closed {
    width: 36px;
  }
  .instruction-row.on-canvas {
    background: var(--color-accent-dark, rgba(88, 166, 255, 0.1));
  }
</style>
