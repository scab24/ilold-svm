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

  onMount(() => {
    terminal = new Terminal({
      cursorBlink: true,
      fontSize: 12,
      fontFamily: 'var(--font-mono), ui-monospace, monospace',
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
    resizeObserver?.disconnect();
    ws?.close();
    terminal?.dispose();
  });
</script>

<div bind:this={terminalEl} class="h-full w-full" />
