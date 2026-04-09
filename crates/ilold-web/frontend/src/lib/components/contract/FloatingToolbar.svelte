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

<div class="float-toolbar" style="left:{toolbarX}px;top:{toolbarY}px" onmousedown={onDown}>
  <a href="/" class="ft-back" title="Back to contracts">←</a>
  <span class="ft-name">{contractName}</span>
  <span class="ft-sep"></span>
  <button class="ft-btn" class:active={mode === 'cfg'} onclick={() => onmodechange('cfg')}>CFG</button>
  <button class="ft-btn" class:active={mode === 'sequences'} onclick={() => onmodechange('sequences')}>Seq</button>
  {#if mode === 'sequences'}
    <span class="ft-sep"></span>
    <button class="ft-btn" class:active={seqDirection === 'TB'} onclick={() => onseqdirection('TB')} title="Vertical">↓</button>
    <button class="ft-btn" class:active={seqDirection === 'LR'} onclick={() => onseqdirection('LR')} title="Horizontal">→</button>
  {/if}
  <span class="ft-sep"></span>
  <button class="ft-btn" onclick={onsearch} title="Cmd+K">Search</button>
  <button class="ft-btn" onclick={oncenter} title="Center all nodes">Center</button>
</div>

<style>
  .float-toolbar {
    position: fixed; z-index: 20;
    display: flex; align-items: center; gap: 3px;
    padding: 5px 10px;
    background: #18181eee; border: 1px solid #252530;
    border-radius: 8px; cursor: grab; user-select: none;
    box-shadow: 0 4px 20px #08080a66; backdrop-filter: blur(12px);
  }
  .float-toolbar:active { cursor: grabbing; }
  .ft-back { color: #6b7a8d; text-decoration: none; font-size: 14px; padding: 3px 6px; border-radius: 4px; }
  .ft-back:hover { background: #252530; color: #8bb8e8; }
  .ft-name { font-size: 13px; font-weight: 700; color: #b8c4d4; padding: 0 4px; }
  .ft-sep { width: 1px; height: 16px; background: #252530; margin: 0 2px; }
  .ft-btn { background: none; border: 1px solid transparent; color: #6b7a8d; padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 11px; }
  .ft-btn:hover { border-color: #5b9bd5; color: #8bb8e8; }
  .ft-btn.active { background: #3a6b9f; border-color: #5b9bd5; color: #dce8f4; }
</style>
