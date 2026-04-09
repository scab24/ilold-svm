import { subscribe } from '$lib/api/ws';
import { postCommand } from '$lib/api/session';
import type {
  SessionStep,
  SessionAddNode,
  SessionHighlight,
  ConnectionEvent,
  SessionViewResponse,
} from '$lib/api/types';

// ── Reactive state (Svelte 5 $state runes) ──────────────────────────────────

let steps = $state<SessionStep[]>([]);
let highlightedFunction = $state<string | null>(null);

function resetState() {
  steps = [];
  highlightedFunction = null;
}

// ── WebSocket subscriptions (created once on module import) ─────────────────

subscribe('session_add_node', (msg: SessionAddNode) => {
  steps = [...steps, {
    function: msg.function,
    access: msg.access,
    step_index: msg.step_index,
  }];
});

subscribe('session_remove_node', () => {
  if (steps.length > 0) {
    steps = steps.slice(0, -1);
  }
});

subscribe('session_clear', resetState);

subscribe('session_highlight', (msg: SessionHighlight) => {
  highlightedFunction = msg.function;
});

subscribe('connection', (event: ConnectionEvent) => {
  if (event.state === 'connected') {
    resync();
  }
});

// ── REST re-sync ────────────────────────────────────────────────────────────

// LIMITATION: REST returns only function names, not full tuples.
// access defaults to "Public", step_index is inferred from array position.
let resyncGen = 0;

async function resync(): Promise<void> {
  const gen = ++resyncGen;
  try {
    const response: SessionViewResponse = await postCommand('Session');
    if (gen !== resyncGen) return; // stale — a newer resync superseded this one

    const view = response.SessionView;
    steps = view.steps.map((name, index) => ({
      function: name,
      access: "Public" as const,
      step_index: index,
    }));
    highlightedFunction = null;
  } catch (e) {
    if (gen === resyncGen) console.warn('Session re-sync failed:', e);
  }
}

// ── Public API ──────────────────────────────────────────────────────────────

export function getSteps(): SessionStep[] {
  return steps;
}

export function getHighlightedFunction(): string | null {
  return highlightedFunction;
}

export function clearSession(): void {
  resetState();
}
