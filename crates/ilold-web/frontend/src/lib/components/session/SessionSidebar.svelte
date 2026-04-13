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
  class="flex flex-col flex-shrink-0 relative h-full"
  style="
    width: {open ? `${sidebarWidth}px` : '28px'};
    background: linear-gradient(180deg, rgba(20, 20, 28, 0.95) 0%, rgba(16, 16, 22, 0.98) 100%);
    border-left: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
  "
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
    class="absolute left-1 top-2 border cursor-pointer px-[3px] py-1 text-[10px] z-5 text-text-muted transition-colors duration-150 hover:text-accent-hover"
    style="
      border-radius: 6px;
      border-color: color-mix(in srgb, var(--color-border) 40%, transparent);
      background: rgba(30, 30, 40, 0.8);
      backdrop-filter: blur(8px);
    "
    onclick={() => open = !open}
  >
    {open ? '▸' : '◂'}
  </button>

  <div class="flex flex-col flex-1 min-h-0" class:hidden={!open}>
    <!-- Tab header — glass effect -->
    <div
      class="flex px-1.5 mb-0"
      style="
        border-bottom: 1px solid color-mix(in srgb, var(--color-border) 40%, transparent);
        background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
        backdrop-filter: blur(16px) saturate(1.8);
        -webkit-backdrop-filter: blur(16px) saturate(1.8);
      "
    >
      <button
        class="flex-1 py-2 bg-transparent border-none text-[10px] font-semibold uppercase tracking-wider cursor-pointer transition-colors duration-150 {activeTab === 'timeline' ? 'text-accent' : 'text-text-muted hover:text-text'}"
        style="border-bottom: 2px solid {activeTab === 'timeline' ? 'var(--color-accent)' : 'transparent'};"
        onclick={() => activeTab = 'timeline'}
      >
        Timeline
      </button>
      <button
        class="flex-1 py-2 bg-transparent border-none text-[10px] font-semibold uppercase tracking-wider cursor-pointer transition-colors duration-150 {activeTab === 'state' ? 'text-accent' : 'text-text-muted hover:text-text'}"
        style="border-bottom: 2px solid {activeTab === 'state' ? 'var(--color-accent)' : 'transparent'};"
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
