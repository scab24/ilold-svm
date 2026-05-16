<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { getProjectMap, type ProjectMap } from '$lib/api/rest';
  import { setSearchContext } from '$lib/stores/search.svelte';
  import { togglePalette, setPaletteCommands, clearPaletteCommands } from '$lib/stores/palette.svelte';
  import type { Command } from '$lib/commands/registry';

  let projectMap: ProjectMap | null = $state(null);
  let error: string | null = $state(null);

  onMount(async () => {
    setSearchContext(null);
    try {
      projectMap = await getProjectMap();
    } catch (e) {
      error = 'Failed to connect. Is "ilold serve" running?';
    }
  });

  $effect(() => {
    if (!projectMap) {
      setPaletteCommands([]);
      return;
    }
    const seen = new Set<string>();
    const cmds: Command[] = [];
    for (const p of projectMap.programs ?? []) {
      if (seen.has(p.name)) continue;
      seen.add(p.name);
      cmds.push({
        id: `program:${p.name}`,
        label: p.name,
        category: 'Contract' as const,
        icon: '◊',
        detail: `${p.instructions.length} ix · ${p.account_types.length} account types`,
        keywords: ['program', 'open', 'navigate', 'solana'],
        run: () => goto(`/contract/${encodeURIComponent(p.name)}`),
      });
    }
    setPaletteCommands(cmds);
  });

  onDestroy(() => clearPaletteCommands());

  let programs = $derived(projectMap?.programs ?? []);
</script>

<div class="fixed inset-0 flex flex-col bg-dark">
  <div class="flex items-center gap-2.5 px-4 py-2 bg-hover border-b border-border-subtle z-10 shrink-0">
    <span class="text-lg font-bold text-text">ilold</span>
    <span class="text-xs text-text-dim">Solana execution path analyzer</span>
    {#if projectMap}
      <span class="text-xs text-text-muted">{programs.length} programs</span>
    {/if}
    <div class="ml-auto flex gap-1">
      <button class="bg-hover border border-border-subtle text-accent-hover px-3 py-1 rounded-sm cursor-pointer text-xs hover:border-accent" onclick={togglePalette}>⌘K Search</button>
    </div>
  </div>

  {#if error}
    <div class="p-6 text-danger">{error}</div>
  {:else if !projectMap}
    <div class="p-6 text-text-muted">Analyzing...</div>
  {:else}
    <div class="flex-1 overflow-y-auto p-6">
      <div class="grid grid-cols-[repeat(auto-fill,minmax(340px,1fr))] gap-4">
        {#each programs as program}
          <div class="bg-hover border border-border-subtle rounded-[10px] overflow-hidden">
            <div class="px-3.5 pt-3 pb-2 border-b border-border-subtle">
              <span class="text-[10px] text-text-muted uppercase tracking-wide">solana program</span>
              <h2 class="text-lg mt-0.5 mb-0"><a class="text-text no-underline hover:text-accent-hover" href="/contract/{program.name}">{program.name}</a></h2>
              <div class="text-[10px] text-text-dim mt-0.5 font-mono truncate">{program.program_id}</div>
            </div>
            <div class="card-section">
              <div class="text-[9px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Instructions</div>
              {#each program.instructions as ix}
                <a href="/contract/{program.name}/{ix.name}" class="flex items-center gap-1.5 px-1 py-1 rounded-sm text-xs text-inherit no-underline hover:bg-border">
                  <span class="size-1.5 rounded-full shrink-0 bg-accent-hover"></span>
                  <span class="text-text font-semibold font-mono flex-1">{ix.name}</span>
                  <span class="text-[10px] text-text-muted">{ix.args_count}a {ix.accounts_count}acc</span>
                  {#if ix.has_pdas}
                    <span class="text-[9px] px-1 py-px rounded-md bg-warning/10 text-warning">pda</span>
                  {/if}
                </a>
              {/each}
            </div>
            {#if program.account_types.length > 0}
              <div class="card-section">
                <div class="text-[9px] text-text-muted uppercase tracking-wide mb-1 font-semibold">Account types</div>
                {#each program.account_types as a}
                  <div class="flex justify-between px-1 py-0.5 text-[11px] font-mono">
                    <span class="text-text">{a.name}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .card-section { padding: 8px 14px; }
  .card-section + .card-section { border-top: 1px solid var(--color-border-subtle); }
</style>
