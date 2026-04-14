import { subscribe } from '$lib/api/ws';
import { getAllScenarios } from '$lib/api/session';
import type {
  SessionStep,
  SessionAddNode,
  SessionRemoveNode,
  SessionClear,
  SessionHighlight,
  ScenarioCreated,
  ScenarioSwitched,
  ScenarioDeleted,
  ScenarioForked,
  ConnectionEvent,
} from '$lib/api/types';

// ── Reactive state (Svelte 5 $state runes) ──────────────────────────────────
//
// Svelte 5 Maps: mutations (set/delete) do NOT trigger reactivity. We reassign
// with `new Map(scenarios)` after each mutation so downstream $derived/$effect
// recomputes. `activeScenario` and `highlightedFunction` are plain scalars.

let scenarios = $state<Map<string, SessionStep[]>>(new Map([['main', []]]));
let activeScenario = $state<string>('main');
let highlightedFunction = $state<string | null>(null);

function resetState() {
  scenarios = new Map([['main', []]]);
  activeScenario = 'main';
  highlightedFunction = null;
}

// ── WebSocket subscriptions (created once on module import) ─────────────────

// session_* events carry a `scenario` field (design §4.1). We route each
// mutation to the matching map entry; if the scenario is unknown we create
// it on the fly — resync() will reconcile on reconnect.

subscribe('session_add_node', (msg: SessionAddNode) => {
  const next = new Map(scenarios);
  const existing = next.get(msg.scenario) ?? [];
  next.set(msg.scenario, [
    ...existing,
    { function: msg.function, access: msg.access, step_index: msg.step_index },
  ]);
  scenarios = next;
});

subscribe('session_remove_node', (msg: SessionRemoveNode) => {
  const existing = scenarios.get(msg.scenario);
  if (!existing || existing.length === 0) return;
  const next = new Map(scenarios);
  next.set(msg.scenario, existing.slice(0, -1));
  scenarios = next;
});

subscribe('session_clear', (msg: SessionClear) => {
  // Scoped clear (design §10.2) — only the target scenario is emptied.
  const next = new Map(scenarios);
  next.set(msg.scenario, []);
  scenarios = next;
});

subscribe('session_highlight', (msg: SessionHighlight) => {
  // highlightedFunction is global in frontend v1; scenario field is ignored.
  highlightedFunction = msg.function;
});

// ── Scenario lifecycle events ───────────────────────────────────────────────

subscribe('scenario_created', (msg: ScenarioCreated) => {
  const next = new Map(scenarios);
  next.set(msg.name, []);
  scenarios = next;
});

subscribe('scenario_switched', (msg: ScenarioSwitched) => {
  activeScenario = msg.to;
});

subscribe('scenario_deleted', (msg: ScenarioDeleted) => {
  const next = new Map(scenarios);
  next.delete(msg.name);
  scenarios = next;
});

subscribe('scenario_forked', (_msg: ScenarioForked) => {
  // Computing the fork locally would require deep-cloning the source steps
  // at at_step; cheaper and safer to resync from backend.
  resync();
});

subscribe('connection', (event: ConnectionEvent) => {
  if (event.state === 'connected') {
    resync();
  }
});

// ── REST re-sync ────────────────────────────────────────────────────────────

let resyncGen = 0;

async function resync(): Promise<void> {
  const gen = ++resyncGen;
  try {
    const response = await getAllScenarios();
    if (gen !== resyncGen) return; // stale — a newer resync superseded this

    const newMap = new Map<string, SessionStep[]>();
    for (const [name, steps] of response.scenarios) {
      // SessionStepView has identical shape to SessionStep — copy fields
      // explicitly so TS infers the narrowed type and future drift breaks here.
      newMap.set(
        name,
        steps.map((s) => ({
          function: s.function,
          access: s.access,
          step_index: s.step_index,
        })),
      );
    }
    // Guarantee 'main' is always present — the store invariant on backend
    // (ScenarioStore::DEFAULT) is "main exists"; if the response is somehow
    // empty, reseed so UI never renders a blank scenario list.
    if (newMap.size === 0) newMap.set('main', []);

    scenarios = newMap;
    activeScenario = response.active || 'main';
    highlightedFunction = null;
  } catch (e) {
    if (gen === resyncGen) console.warn('Session re-sync failed:', e);
  }
}

// ── Public API ──────────────────────────────────────────────────────────────

export function getScenarios(): Map<string, SessionStep[]> {
  return scenarios;
}

export function getActiveScenario(): string {
  return activeScenario;
}

export function getScenarioSteps(name: string): SessionStep[] {
  return scenarios.get(name) ?? [];
}

// Back-compat: returns active scenario's steps. Existing consumers
// (SessionTimeline, StatePanel, contract page) keep working unchanged.
export function getSteps(): SessionStep[] {
  return getScenarioSteps(activeScenario);
}

export function getHighlightedFunction(): string | null {
  return highlightedFunction;
}

export function clearSession(): void {
  resetState();
}
