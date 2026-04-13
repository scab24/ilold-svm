<script lang="ts">
  import EmbeddedTerminal from './EmbeddedTerminal.svelte';
  import SessionTimeline from './SessionTimeline.svelte';
  import StatePanel from './StatePanel.svelte';

  let { contract }: { contract: string } = $props();

  let open = $state(true);
  let activeTab: 'timeline' | 'state' = $state('timeline');

  // Resizable sidebar width (drag handle on left edge)
  let sidebarWidth = $state(480);
  const MIN_WIDTH = 320;
  const MAX_WIDTH = 900;
  let draggingWidth = $state(false);

  function onWidthDragStart(e: MouseEvent) {
    e.preventDefault();
    draggingWidth = true;
    document.body.style.userSelect = 'none';
    const startX = e.clientX;
    const startW = sidebarWidth;

    function onMove(ev: MouseEvent) {
      // Sidebar grows to the left, so drag left = wider
      const delta = startX - ev.clientX;
      sidebarWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW + delta));
    }
    function onUp() {
      draggingWidth = false;
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }
</script>

<div
  class="flex flex-col flex-shrink-0 bg-dark border-l border-border relative h-full"
  style:width={open ? `${sidebarWidth}px` : '28px'}
>
  <!-- Sidebar width drag handle (left edge) -->
  {#if open}
    <div
      class="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize z-10 {draggingWidth ? 'bg-accent-dark' : 'hover:bg-surface-alt'}"
      onmousedown={onWidthDragStart}
      role="separator"
      aria-orientation="vertical"
    ></div>
  {/if}

  <button
    class="absolute left-1 top-2 bg-surface border border-border border-r-0 rounded-l-[4px] text-text-muted cursor-pointer px-[3px] py-1 text-[10px] z-5 hover:text-accent-hover"
    onclick={() => open = !open}
  >
    {open ? '▸' : '◂'}
  </button>

  <div class="flex flex-col flex-1 min-h-0" class:hidden={!open}>
    <div class="flex border-b border-border px-1 mb-1">
      <button
        class="flex-1 py-1.5 bg-transparent border-none border-b-2 text-[10px] font-semibold uppercase tracking-[0.5px] cursor-pointer {activeTab === 'timeline' ? 'text-accent border-b-accent' : 'text-text-muted border-b-transparent hover:text-text'}"
        onclick={() => activeTab = 'timeline'}
      >
        Timeline
      </button>
      <button
        class="flex-1 py-1.5 bg-transparent border-none border-b-2 text-[10px] font-semibold uppercase tracking-[0.5px] cursor-pointer {activeTab === 'state' ? 'text-accent border-b-accent' : 'text-text-muted border-b-transparent hover:text-text'}"
        onclick={() => activeTab = 'state'}
      >
        State
      </button>
    </div>

    <div class="flex-1 overflow-y-auto min-h-0 px-1">
      {#if activeTab === 'timeline'}
        <SessionTimeline {contract} />
      {:else}
        <StatePanel {contract} />
      {/if}
    </div>
  </div>

  <!-- Floating terminal (positions itself fixed, outside sidebar flow) -->
  <EmbeddedTerminal />
</div>
