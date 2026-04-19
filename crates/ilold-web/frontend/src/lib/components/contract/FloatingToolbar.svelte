<script lang="ts">
  import { onMount } from 'svelte';
  import { toggleTerminal, isTerminalVisible } from '$lib/stores/terminal.svelte';

  let {
    contractName,
    mode,
    seqDirection,
    onmodechange,
    onsearch,
    oncenter,
    onseqdirection,
    onsessionback,
    onsessionclear,
  }: {
    contractName: string;
    mode: 'cfg' | 'sequences' | 'session';
    seqDirection: 'TB' | 'LR';
    onmodechange: (mode: 'cfg' | 'sequences' | 'session') => void;
    onsearch: () => void;
    oncenter: () => void;
    onseqdirection: (dir: 'TB' | 'LR') => void;
    /** Remove the last step of the active scenario (like REPL `b`). */
    onsessionback: () => void;
    /** Clear every step of the active scenario (like REPL `cl`). */
    onsessionclear: () => void;
  } = $props();

  let toolbarX = $state(0);
  let toolbarY = $state(10);
  let dragging = $state(false);
  let offX = 0;
  let offY = 0;

  onMount(() => {
    toolbarX = Math.floor(window.innerWidth / 2 - 150);
  });

  function onDown(e: MouseEvent) {
    if ((e.target as HTMLElement).tagName === 'BUTTON' || (e.target as HTMLElement).tagName === 'A') return;
    dragging = true;
    offX = e.clientX - toolbarX;
    offY = e.clientY - toolbarY;
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function onMove(e: MouseEvent) {
    if (!dragging) return;
    toolbarX = e.clientX - offX;
    toolbarY = Math.max(0, e.clientY - offY);
  }

  function onUp() {
    dragging = false;
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  }
</script>

<div
  class="fixed z-20 flex items-center gap-1 px-1.5 py-1 select-none"
  class:cursor-grabbing={dragging}
  class:cursor-grab={!dragging}
  style="
    left:{toolbarX}px;
    top:{toolbarY}px;
    border-radius: 12px;
    border: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
    backdrop-filter: blur(16px) saturate(1.8);
    -webkit-backdrop-filter: blur(16px) saturate(1.8);
    box-shadow:
      0 8px 32px -8px rgba(0, 0, 0, 0.4),
      0 4px 16px -4px rgba(0, 0, 0, 0.2),
      0 0 0 1px rgba(91, 155, 213, 0.05),
      0 0 40px -16px rgba(91, 155, 213, 0.06);
  "
  onmousedown={onDown}
>
  <a
    href="/"
    class="text-text-muted no-underline text-sm px-2 py-1 transition-colors duration-150 hover:text-accent-hover"
    style="border-radius: 6px;"
    title="Back to contracts"
  >←</a>
  <span class="text-[11px] font-bold text-text px-1.5 tracking-wide uppercase">{contractName}</span>

  <!-- Separator -->
  <span class="mx-0.5" style="width: 1px; height: 18px; background: color-mix(in srgb, var(--color-border) 50%, transparent);"></span>

  <button
    class="bg-transparent border text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover {mode === 'cfg' ? 'text-accent-light' : ''}"
    style="
      border-radius: 6px;
      border-color: {mode === 'cfg' ? 'var(--color-accent)' : 'transparent'};
      background: {mode === 'cfg' ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
    "
    onclick={() => onmodechange('cfg')}
  >CFG</button>
  <button
    class="bg-transparent border text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover {mode === 'sequences' ? 'text-accent-light' : ''}"
    style="
      border-radius: 6px;
      border-color: {mode === 'sequences' ? 'var(--color-accent)' : 'transparent'};
      background: {mode === 'sequences' ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
    "
    onclick={() => onmodechange('sequences')}
  >Seq</button>
  <button
    class="bg-transparent border text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover {mode === 'session' ? 'text-accent-light' : ''}"
    style="
      border-radius: 6px;
      border-color: {mode === 'session' ? 'var(--color-accent)' : 'transparent'};
      background: {mode === 'session' ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
    "
    onclick={() => onmodechange('session')}
    title="Session mode: click in the sidebar to add steps; only scenarios render on the canvas"
  >Session</button>

  {#if mode === 'sequences'}
    <span class="mx-0.5" style="width: 1px; height: 18px; background: color-mix(in srgb, var(--color-border) 50%, transparent);"></span>
    <button
      class="bg-transparent border text-text-muted px-2 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover {seqDirection === 'TB' ? 'text-accent-light' : ''}"
      style="
        border-radius: 6px;
        border-color: {seqDirection === 'TB' ? 'var(--color-accent)' : 'transparent'};
        background: {seqDirection === 'TB' ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
      "
      onclick={() => onseqdirection('TB')} title="Vertical"
    >↓</button>
    <button
      class="bg-transparent border text-text-muted px-2 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover {seqDirection === 'LR' ? 'text-accent-light' : ''}"
      style="
        border-radius: 6px;
        border-color: {seqDirection === 'LR' ? 'var(--color-accent)' : 'transparent'};
        background: {seqDirection === 'LR' ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
      "
      onclick={() => onseqdirection('LR')} title="Horizontal"
    >→</button>
  {/if}

  <span class="mx-0.5" style="width: 1px; height: 18px; background: color-mix(in srgb, var(--color-border) 50%, transparent);"></span>

  <button
    class="bg-transparent border border-transparent text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover"
    style="border-radius: 6px;"
    onclick={onsessionback} title="Back — remove last step of active scenario"
  >↶</button>
  <button
    class="bg-transparent border border-transparent text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-danger"
    style="border-radius: 6px;"
    onclick={onsessionclear} title="Clear — remove all steps of active scenario"
  >🗑</button>

  <span class="mx-0.5" style="width: 1px; height: 18px; background: color-mix(in srgb, var(--color-border) 50%, transparent);"></span>

  <button
    class="bg-transparent border border-transparent text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover"
    style="border-radius: 6px;"
    onclick={onsearch} title="Cmd+K"
  >Search</button>
  <button
    class="bg-transparent border border-transparent text-text-muted px-2.5 py-1 cursor-pointer text-[11px] transition-colors duration-150 hover:text-accent-hover"
    style="border-radius: 6px;"
    onclick={oncenter} title="Center all nodes"
  >Center</button>

  <span class="mx-0.5" style="width: 1px; height: 18px; background: color-mix(in srgb, var(--color-border) 50%, transparent);"></span>

  <!-- Terminal toggle button -->
  <button
    class="bg-transparent border text-text-muted px-2 py-1 cursor-pointer text-[11px] font-mono transition-colors duration-150 hover:text-accent-hover"
    style="
      border-radius: 6px;
      border-color: {isTerminalVisible() ? 'var(--color-accent)' : 'transparent'};
      background: {isTerminalVisible() ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)' : 'transparent'};
      color: {isTerminalVisible() ? 'var(--color-accent-light)' : ''};
    "
    onclick={toggleTerminal}
    title={isTerminalVisible() ? 'Hide terminal' : 'Show terminal'}
  >&gt;_</button>
</div>
