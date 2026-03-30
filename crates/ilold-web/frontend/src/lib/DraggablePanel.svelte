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
  let w = $state(width);
  let h = $state(0);
  let panelEl: HTMLDivElement;
  let mode: 'idle' | 'drag' | 'resize-se' | 'resize-e' | 'resize-s' = 'idle';
  let startX = 0;
  let startY = 0;
  let startW = 0;
  let startH = 0;

  function onDragStart(e: MouseEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === 'INPUT' || tag === 'BUTTON' || tag === 'A') return;
    mode = 'drag';
    startX = e.clientX - posX;
    startY = e.clientY - posY;
    listen();
  }

  function onResizeStart(which: 'resize-se' | 'resize-e' | 'resize-s', e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    mode = which;
    startX = e.clientX;
    startY = e.clientY;
    startW = w;
    startH = h > 0 ? h : panelEl.offsetHeight;
    listen();
  }

  function onMove(e: MouseEvent) {
    if (mode === 'drag') {
      posX = Math.max(0, Math.min(window.innerWidth - 100, e.clientX - startX));
      posY = Math.max(0, Math.min(window.innerHeight - 40, e.clientY - startY));
    } else if (mode === 'resize-se') {
      w = Math.max(180, startW + (e.clientX - startX));
      h = Math.max(100, startH + (e.clientY - startY));
    } else if (mode === 'resize-e') {
      w = Math.max(180, startW + (e.clientX - startX));
    } else if (mode === 'resize-s') {
      h = Math.max(100, startH + (e.clientY - startY));
    }
  }

  function onEnd() {
    mode = 'idle';
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onEnd);
  }

  function listen() {
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onEnd);
  }
</script>

<div
  class="draggable-panel"
  bind:this={panelEl}
  style="left:{posX}px; top:{posY}px; width:{w}px; {h > 0 ? `height:${h}px;` : ''}"
>
  <div class="drag-header" onmousedown={onDragStart}>
    <span class="drag-title">{title}</span>
    {#if onclose}
      <button class="drag-close" onclick={onclose}>✕</button>
    {/if}
  </div>
  <div class="drag-body">
    {@render children()}
  </div>
  <!-- Resize edges -->
  <div class="resize-e" onmousedown={(e) => onResizeStart('resize-e', e)}></div>
  <div class="resize-s" onmousedown={(e) => onResizeStart('resize-s', e)}></div>
  <div class="resize-se" onmousedown={(e) => onResizeStart('resize-se', e)}></div>
</div>

<style>
  .draggable-panel {
    position: fixed;
    background: #18181eee;
    border: 1px solid #252530;
    border-radius: 10px;
    z-index: 50;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 24px #08080a66;
    backdrop-filter: blur(12px);
    max-height: calc(100vh - 20px);
    overflow: hidden;
    min-width: 180px;
  }

  .drag-header {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    border-bottom: 1px solid #252530;
    cursor: grab;
    user-select: none;
    gap: 6px;
    flex-shrink: 0;
  }
  .drag-header:active { cursor: grabbing; }

  .drag-title {
    font-size: 12px;
    font-weight: 600;
    color: #8bb8e8;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .drag-close {
    background: none;
    border: none;
    color: #4a5568;
    cursor: pointer;
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 3px;
  }
  .drag-close:hover { background: #252530; color: #b8c4d4; }

  .drag-body {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #333340 transparent;
    padding-bottom: 14px;
  }

  /* Resize handles — edges and corner */
  .resize-e {
    position: absolute; top: 10px; right: -3px; bottom: 10px; width: 6px;
    cursor: ew-resize;
  }
  .resize-s {
    position: absolute; left: 10px; bottom: -3px; right: 10px; height: 6px;
    cursor: ns-resize;
  }
  .resize-se {
    position: absolute; bottom: 0; right: 0; width: 18px; height: 18px;
    cursor: nwse-resize; border-radius: 0 0 10px 0;
  }
  .resize-se::after {
    content: '';
    position: absolute; bottom: 4px; right: 4px;
    width: 8px; height: 8px;
    border-right: 2px solid #4a5568;
    border-bottom: 2px solid #4a5568;
    opacity: 0.6;
  }
  .resize-se:hover::after { opacity: 1; border-color: #8bb8e8; }
</style>
