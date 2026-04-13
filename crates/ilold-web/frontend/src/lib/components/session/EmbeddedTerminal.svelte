<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';

  let terminalEl: HTMLDivElement;
  let terminal: Terminal;
  let fitAddon: FitAddon;
  let ws: WebSocket | null = null;
  let resizeObserver: ResizeObserver | null = null;

  // Floating panel state
  let minimized = $state(false);

  // Position — set properly in onMount; defaults are SSR-safe placeholders
  let posX = $state(0);
  let posY = $state(0);

  // Size
  let panelWidth = $state(600);
  let panelHeight = $state(300);
  const MIN_W = 400;
  const MIN_H = 200;
  const TITLE_BAR_H = 32;

  // Drag state
  let dragging = $state(false);

  function onDragStart(e: MouseEvent) {
    e.preventDefault();
    dragging = true;
    document.body.style.userSelect = 'none';
    const startX = e.clientX;
    const startY = e.clientY;
    const startPosX = posX;
    const startPosY = posY;

    function onMove(ev: MouseEvent) {
      posX = startPosX + (ev.clientX - startX);
      posY = startPosY + (ev.clientY - startY);
      clampToViewport();
    }
    function onUp() {
      dragging = false;
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function onResizeStart(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    document.body.style.userSelect = 'none';
    const startX = e.clientX;
    const startY = e.clientY;
    const startW = panelWidth;
    const startH = panelHeight;

    function onMove(ev: MouseEvent) {
      const maxW = window.innerWidth - posX - 16;
      const maxH = window.innerHeight - posY - 16;
      panelWidth = Math.min(maxW, Math.max(MIN_W, startW + (ev.clientX - startX)));
      panelHeight = Math.min(maxH, Math.max(MIN_H, startH + (ev.clientY - startY)));
    }
    function onUp() {
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function toggleMinimize() {
    minimized = !minimized;
    if (!minimized) {
      // Re-fit terminal after restoring
      requestAnimationFrame(() => fitAddon?.fit());
    }
  }

  function clampToViewport() {
    posX = Math.max(0, Math.min(posX, window.innerWidth - panelWidth));
    posY = Math.max(0, Math.min(posY, window.innerHeight - (minimized ? TITLE_BAR_H : panelHeight)));
  }

  onMount(() => {
    // Set initial position (bottom-right with 16px padding)
    posX = window.innerWidth - panelWidth - 16;
    posY = window.innerHeight - panelHeight - 16;

    window.addEventListener('resize', clampToViewport);

    terminal = new Terminal({
      cursorBlink: true,
      fontSize: 11,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', var(--font-mono), ui-monospace, monospace",
      lineHeight: 1.15,
      theme: {
        background: '#0d1117',
        foreground: '#c9d1d9',
        cursor: '#58a6ff',
        selectionBackground: '#264f78',
      },
    });

    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(terminalEl);
    // Delay fit to ensure container has layout dimensions
    requestAnimationFrame(() => fitAddon.fit());

    // Connect to PTY WebSocket
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${location.host}/ws/pty`);
    ws.binaryType = 'arraybuffer';

    ws.onopen = () => {
      // Send initial size so the PTY starts with correct dimensions
      const { cols, rows } = terminal;
      ws!.send(JSON.stringify({ type: 'resize', cols, rows }));
    };

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        terminal.write(new Uint8Array(event.data));
      } else {
        terminal.write(event.data);
      }
    };

    ws.onclose = () => {
      terminal.write('\r\n\x1b[90m[Disconnected]\x1b[0m\r\n');
    };

    // User input → PTY stdin
    terminal.onData((data) => {
      if (ws?.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode(data));
      }
    });

    // Resize → notify PTY
    terminal.onResize(({ cols, rows }) => {
      if (ws?.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }));
      }
    });

    // Auto-fit when container resizes
    resizeObserver = new ResizeObserver(() => fitAddon.fit());
    resizeObserver.observe(terminalEl);
  });

  onDestroy(() => {
    window.removeEventListener('resize', clampToViewport);
    resizeObserver?.disconnect();
    ws?.close();
    terminal?.dispose();
  });
</script>

<div
  class="fixed z-50 flex flex-col rounded-lg border border-border shadow-xl overflow-hidden"
  style="left: {posX}px; top: {posY}px; width: {panelWidth}px; height: {minimized ? TITLE_BAR_H : panelHeight}px;"
>
  <!-- Title bar (drag handle) -->
  <div
    class="flex items-center justify-between px-3 h-8 shrink-0 bg-surface border-b border-border select-none {dragging ? 'cursor-grabbing' : 'cursor-grab'}"
    onmousedown={onDragStart}
    role="toolbar"
  >
    <span class="text-[11px] font-semibold text-text-muted tracking-wide uppercase">Terminal</span>
    <div class="flex items-center gap-1">
      <button
        class="bg-transparent border-none text-text-muted hover:text-accent-hover cursor-pointer text-xs px-1.5 py-0.5 rounded hover:bg-surface-alt"
        onclick={toggleMinimize}
        title={minimized ? 'Restore' : 'Minimize'}
      >
        {minimized ? '▲' : '▼'}
      </button>
    </div>
  </div>

  <!-- Terminal body (use hidden instead of {#if} to preserve xterm DOM) -->
  <div class="flex-1 min-h-0 bg-[#0d1117]" class:hidden={minimized}>
    <div bind:this={terminalEl} class="h-full w-full" />
  </div>

  <!-- Resize handle (bottom-right corner) -->
  {#if !minimized}
    <div
      class="absolute bottom-0 right-0 w-4 h-4 cursor-nwse-resize z-10"
      onmousedown={onResizeStart}
      role="separator"
      aria-label="Resize terminal"
    >
      <svg class="w-3 h-3 text-text-muted absolute bottom-0.5 right-0.5" viewBox="0 0 12 12" fill="currentColor">
        <circle cx="10" cy="10" r="1.5" />
        <circle cx="6" cy="10" r="1.5" />
        <circle cx="10" cy="6" r="1.5" />
      </svg>
    </div>
  {/if}
</div>
