<script lang="ts">
  let { children, title = '', x = 12, y = 50, width = 300, onclose }: {
    children: any;
    title?: string;
    x?: number;
    y?: number;
    width?: number;
    onclose?: () => void;
  } = $props();

  let posX = $state(x);
  let posY = $state(y);
  let dragging = false;
  let offsetX = 0;
  let offsetY = 0;
  let panelEl: HTMLDivElement;

  function onMouseDown(e: MouseEvent) {
    if ((e.target as HTMLElement).tagName === 'INPUT' ||
        (e.target as HTMLElement).tagName === 'BUTTON' ||
        (e.target as HTMLElement).tagName === 'A') return;
    dragging = true;
    offsetX = e.clientX - posX;
    offsetY = e.clientY - posY;
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  function onMouseMove(e: MouseEvent) {
    if (!dragging) return;
    posX = Math.max(0, e.clientX - offsetX);
    posY = Math.max(0, e.clientY - offsetY);
  }

  function onMouseUp() {
    dragging = false;
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  }
</script>

<div
  class="draggable-panel"
  bind:this={panelEl}
  style="left:{posX}px; top:{posY}px; width:{width}px;"
>
  <div class="drag-header" onmousedown={onMouseDown}>
    <span class="drag-title">{title}</span>
    <div class="drag-handle">⠿</div>
    {#if onclose}
      <button class="drag-close" onclick={onclose}>✕</button>
    {/if}
  </div>
  <div class="drag-body">
    {@render children()}
  </div>
</div>

<style>
  .draggable-panel {
    position: fixed;
    background: #161b22ee;
    border: 1px solid #30363d;
    border-radius: 10px;
    z-index: 50;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 20px #00000044;
    backdrop-filter: blur(12px);
    max-height: calc(100vh - 60px);
    overflow: hidden;
  }

  .drag-header {
    display: flex;
    align-items: center;
    padding: 5px 8px;
    border-bottom: 1px solid #21262d;
    cursor: grab;
    user-select: none;
    gap: 6px;
  }

  .drag-header:active { cursor: grabbing; }

  .drag-title {
    font-size: 11px;
    font-weight: 600;
    color: #8b949e;
    flex: 1;
  }

  .drag-handle {
    color: #484f58;
    font-size: 12px;
    letter-spacing: 1px;
  }

  .drag-close {
    background: none;
    border: none;
    color: #484f58;
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
    border-radius: 3px;
  }
  .drag-close:hover { background: #21262d; color: #f0f6fc; }

  .drag-body {
    flex: 1;
    overflow-y: auto;
  }
</style>
