<script lang="ts">
  import { onMount } from 'svelte';

  let {
    contractName,
    mode,
    seqDirection,
    onmodechange,
    onsearch,
    oncenter,
    onseqdirection,
  }: {
    contractName: string;
    mode: 'cfg' | 'sequences';
    seqDirection: 'TB' | 'LR';
    onmodechange: (mode: 'cfg' | 'sequences') => void;
    onsearch: () => void;
    oncenter: () => void;
    onseqdirection: (dir: 'TB' | 'LR') => void;
  } = $props();

  let toolbarX = $state(0);
  let toolbarY = $state(10);
  let dragging = false;
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
  class="fixed z-20 flex items-center gap-[3px] px-2.5 py-[5px] bg-surface/95 border border-border rounded-lg cursor-grab select-none shadow-[0_4px_20px_var(--color-shadow)] backdrop-blur-md active:cursor-grabbing"
  style="left:{toolbarX}px;top:{toolbarY}px"
  onmousedown={onDown}
>
  <a href="/" class="text-text-muted no-underline text-sm px-1.5 py-[3px] rounded-sm hover:bg-border hover:text-accent-hover" title="Back to contracts">←</a>
  <span class="text-[13px] font-bold text-text px-1">{contractName}</span>
  <span class="w-px h-4 bg-border mx-0.5"></span>
  <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover {mode === 'cfg' ? 'bg-accent-dark border-accent text-accent-light' : ''}" onclick={() => onmodechange('cfg')}>CFG</button>
  <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover {mode === 'sequences' ? 'bg-accent-dark border-accent text-accent-light' : ''}" onclick={() => onmodechange('sequences')}>Seq</button>
  {#if mode === 'sequences'}
    <span class="w-px h-4 bg-border mx-0.5"></span>
    <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover {seqDirection === 'TB' ? 'bg-accent-dark border-accent text-accent-light' : ''}" onclick={() => onseqdirection('TB')} title="Vertical">↓</button>
    <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover {seqDirection === 'LR' ? 'bg-accent-dark border-accent text-accent-light' : ''}" onclick={() => onseqdirection('LR')} title="Horizontal">→</button>
  {/if}
  <span class="w-px h-4 bg-border mx-0.5"></span>
  <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover" onclick={onsearch} title="Cmd+K">Search</button>
  <button class="bg-transparent border border-transparent text-text-muted px-2 py-[3px] rounded-sm cursor-pointer text-[11px] hover:border-accent hover:text-accent-hover" onclick={oncenter} title="Center all nodes">Center</button>
</div>
