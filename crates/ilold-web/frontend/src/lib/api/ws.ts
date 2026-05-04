import type {
  TopicMap,
  ServerMessage,
  SearchCallbacks,
  SearchOptions,
  SearchError,
  ConnectionEvent,
  ConnectionState,
  SessionEventCallbacks,
} from './types';

// ── Singleton state ─────────────────────────────────────────────────────────

let socket: WebSocket | null = null;
let intentionalClose = false;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let backoff = 1000;
const INITIAL_BACKOFF = 1000;
const MAX_BACKOFF = 30_000;
const MAX_QUEUE = 100;
let messageQueue: string[] = [];

// Subscribers persist across connection cycles — never cleared on disconnect (S30)
const subscribers = new Map<string, Set<(msg: unknown) => void>>();

// Backend ServerMessage variants — anything outside this set is truly unknown
const knownTopics: ReadonlySet<string> = new Set([
  'search_result',
  'search_complete',
  'error',
  'session_add_node',
  'session_remove_node',
  'session_clear',
  'session_highlight',
]);

// ── Pub/Sub core ────────────────────────────────────────────────────────────

export function subscribe<T extends keyof TopicMap>(
  topic: T,
  cb: (msg: TopicMap[T]) => void,
): () => void {
  let set = subscribers.get(topic);
  if (!set) {
    set = new Set();
    subscribers.set(topic, set);
  }
  set.add(cb as (msg: unknown) => void);

  ensureSocket();

  return () => {
    set!.delete(cb as (msg: unknown) => void);
    if (set!.size === 0) subscribers.delete(topic);
  };
}

function dispatch(topic: string, msg: unknown): void {
  const set = subscribers.get(topic);
  if (!set) return;
  for (const cb of set) cb(msg);
}

// ── Socket lifecycle ────────────────────────────────────────────────────────

function ensureSocket(): WebSocket {
  if (socket) {
    const rs = socket.readyState;
    if (rs === WebSocket.OPEN || rs === WebSocket.CONNECTING) return socket;
    socket = null;
  }

  // Reset after teardown() so future sockets can auto-reconnect
  intentionalClose = false;

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = `${protocol}//${window.location.host}/ws`;

  socket = new WebSocket(wsUrl);
  socket.onopen = handleOpen;
  socket.onmessage = handleMessage;
  socket.onclose = handleClose;
  // onerror always followed by onclose — handleClose does all cleanup
  socket.onerror = () => {};

  return socket;
}

function handleMessage(event: MessageEvent): void {
  let msg: ServerMessage;
  try {
    msg = JSON.parse(event.data);
  } catch (e) {
    console.warn('WS malformed JSON:', e);
    return;
  }

  // Discriminant from #[serde(tag = "type")] — internally tagged enum
  const topic = msg.type;

  if (!subscribers.has(topic)) {
    if (!knownTopics.has(topic)) {
      console.warn(`Unknown WS message type: ${topic}`);
    }
    return;
  }

  dispatch(topic, msg);
}

function handleOpen(): void {
  backoff = INITIAL_BACKOFF;

  const queued = messageQueue.splice(0);
  for (const json of queued) socket!.send(json);

  dispatch('connection', { state: 'connected' } as ConnectionEvent);
}

function handleClose(): void {
  socket = null;

  // Synthetic error so active search() calls reset via onError (S6)
  dispatch('error', {
    type: 'error',
    message: 'WebSocket disconnected',
    disconnected: true,
  } as SearchError);

  dispatch('connection', { state: 'disconnected' } as ConnectionEvent);

  if (!intentionalClose) scheduleReconnect();
}

// ── Reconnection + teardown ─────────────────────────────────────────────────

function scheduleReconnect(): void {
  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }

  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    ensureSocket();
  }, backoff);

  backoff = Math.min(backoff * 2, MAX_BACKOFF);
}

export function teardown(): void {
  intentionalClose = true;

  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }

  messageQueue.length = 0;

  if (socket) {
    socket.close();
    socket = null;
  }
}

export function send(msg: object): void {
  const json = JSON.stringify(msg);

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(json);
    return;
  }

  if (messageQueue.length >= MAX_QUEUE) {
    messageQueue.shift();
    console.warn('WS outbound queue overflow — oldest message dropped');
  }
  messageQueue.push(json);

  ensureSocket();
}

export function getConnectionState(): ConnectionState {
  if (!socket) return 'disconnected';

  switch (socket.readyState) {
    case WebSocket.CONNECTING:
      return 'connecting';
    case WebSocket.OPEN:
      return 'connected';
    case WebSocket.CLOSING:
    case WebSocket.CLOSED:
    default:
      return 'disconnected';
  }
}

// ── Convenience: search ─────────────────────────────────────────────────────

/**
 * Scoped search subscriptions that auto-unsubscribe on complete/error.
 *
 * Limitation: no request-ID correlation from the backend. Overlapping searches
 * will cross-deliver results until the first auto-unsubscribes. Mitigations:
 * 1. Auto-unsub on search_complete keeps the overlap window small
 * 2. CommandPalette debounces at 250ms — one active search at a time
 * 3. Raw subscribe('search_result') gets ALL results — filter if needed
 */
export function search(
  query: string,
  callbacks: SearchCallbacks,
  options?: SearchOptions,
): () => void {
  const unsubs: Array<() => void> = [];

  function cleanup() {
    for (const u of unsubs) u();
    unsubs.length = 0;
  }

  unsubs.push(
    subscribe('search_result', (msg) => {
      callbacks.onResult(msg);
    }),
  );

  unsubs.push(
    subscribe('search_complete', (msg) => {
      callbacks.onComplete(msg.total);
      cleanup();
    }),
  );

  unsubs.push(
    subscribe('error', (msg) => {
      callbacks.onError(msg.message);
      cleanup();
    }),
  );

  send({
    type: 'search',
    query,
    contract: options?.contract,
    function: options?.function,
  });

  return cleanup;
}

// ── Convenience: session events ─────────────────────────────────────────────

export function onSessionEvent(callbacks: SessionEventCallbacks): () => void {
  const unsubs: Array<() => void> = [];

  if (callbacks.onAddNode) {
    unsubs.push(subscribe('session_add_node', callbacks.onAddNode));
  }
  if (callbacks.onRemoveNode) {
    unsubs.push(subscribe('session_remove_node', callbacks.onRemoveNode));
  }
  if (callbacks.onClear) {
    unsubs.push(subscribe('session_clear', callbacks.onClear));
  }
  if (callbacks.onHighlight) {
    unsubs.push(subscribe('session_highlight', callbacks.onHighlight));
  }

  return () => {
    for (const u of unsubs) u();
  };
}
