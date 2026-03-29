export interface SearchResult {
  contract: string;
  function: string;
  path_id: number;
  terminal: string;
  matches: { field: string; value: string }[];
  depth: number;
}

type MessageHandler = {
  onResult: (result: SearchResult) => void;
  onComplete: (total: number) => void;
  onError: (message: string) => void;
};

let socket: WebSocket | null = null;
let handler: MessageHandler | null = null;

function getSocket(): WebSocket {
  if (socket && socket.readyState === WebSocket.OPEN) return socket;

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = `${protocol}//${window.location.host}/ws`;
  socket = new WebSocket(wsUrl);

  socket.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (!handler) return;

    switch (msg.type) {
      case 'search_result':
        handler.onResult(msg);
        break;
      case 'search_complete':
        handler.onComplete(msg.total);
        break;
      case 'error':
        handler.onError(msg.message);
        break;
    }
  };

  socket.onclose = () => { socket = null; };
  socket.onerror = () => { socket = null; };

  return socket;
}

export function search(
  query: string,
  callbacks: MessageHandler,
  options?: { contract?: string; function?: string }
) {
  handler = callbacks;
  const ws = getSocket();

  const send = () => {
    ws.send(JSON.stringify({
      type: 'search',
      query,
      contract: options?.contract,
      function: options?.function,
    }));
  };

  if (ws.readyState === WebSocket.OPEN) {
    send();
  } else {
    ws.addEventListener('open', send, { once: true });
  }
}
