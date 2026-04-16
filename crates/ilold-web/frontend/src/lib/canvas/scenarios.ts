import type { SessionStep, AccessLevel, ForkOrigin } from '$lib/api/types';

// ── Composed tree types ─────────────────────────────────────────────────────

export interface ComposedNode {
  id: string;
  stepIndex: number;
  /** Horizontal lane index. Each scenario owns a lane by insertion order
   *  (main = 0, first fork = 1, and so on). */
  lane: number;
  function: string;
  access: AccessLevel;
  _sessionStep: true;
  /** The scenario that OWNS this node. Inherited steps are not emitted as
   *  separate nodes — they live on the origin's lane and the fork connects
   *  to them via a fork edge. */
  _scenario: string;
  /** Every scenario whose full path passes through this node. Computed by
   *  chasing each scenario's ancestry: if alt1 was forked from main at N,
   *  alt1's path includes main:step:0..N-1 + alt1:step:N..end. */
  _scenariosPassingThrough: string[];
}

export interface ComposedEdge {
  id: string;
  source: string;
  target: string;
  /** The scenario this edge belongs to. Within-scenario edges use the
   *  owning scenario's name; fork edges carry the forked scenario. */
  _scenario: string;
  /** Fork edges connect an origin's last inherited step to the fork's first
   *  divergent step. Rendered with a distinct style (curved, muted). */
  _forkEdge?: true;
}

export interface ComposedTree {
  nodes: ComposedNode[];
  edges: ComposedEdge[];
}

// ── Fork-aware composition ──────────────────────────────────────────────────
//
// Each scenario emits ONLY its divergent-tail nodes (`steps[at_step..end]`).
// The inherited prefix is visually reused from the origin's lane. A single
// fork edge connects `origin:step:{at_step-1}` to `self:step:{at_step}` so
// the canvas reads as a tree where branches emerge from their origin node.
//
// Chain forks (alt2 forked from alt1) work transitively: alt2's inherited
// prefix is alt1's path up to at_step, which in turn includes main's prefix
// if alt1 was a fork. Path membership is computed by chasing ancestry.

export function composeScenarioTree(
  scenarios: Map<string, SessionStep[]>,
  forkOrigins: Map<string, ForkOrigin>,
): ComposedTree {
  const nodes: ComposedNode[] = [];
  const edges: ComposedEdge[] = [];
  if (scenarios.size === 0) return { nodes, edges };

  const scenarioNames = Array.from(scenarios.keys());
  const laneIndex = new Map(scenarioNames.map((n, i) => [n, i]));

  // Emit per-scenario nodes + within-scenario edges + fork edge.
  for (const name of scenarioNames) {
    const steps = scenarios.get(name)!;
    const origin = forkOrigins.get(name);
    const renderFrom = origin?.at_step ?? 0;
    const lane = laneIndex.get(name)!;

    for (let i = renderFrom; i < steps.length; i++) {
      const step = steps[i];
      const id = nodeId(name, i);
      nodes.push({
        id,
        stepIndex: i,
        lane,
        function: step.function,
        access: step.access,
        _sessionStep: true,
        _scenario: name,
        _scenariosPassingThrough: [], // filled in the second pass below
      });
      if (i > renderFrom) {
        edges.push({
          id: `session-edge:${name}:${i - 1}→${i}`,
          source: nodeId(name, i - 1),
          target: id,
          _scenario: name,
        });
      }
    }

    // Fork edge — only if the origin exists and has steps up to at_step.
    if (origin && origin.at_step > 0 && steps.length > renderFrom) {
      const originSteps = scenarios.get(origin.scenario);
      // Guard: origin may have been deleted or truncated since the fork —
      // skip the edge and render this scenario standalone.
      if (originSteps && originSteps.length >= origin.at_step) {
        const sourceNodeId = resolveOriginNodeId(
          origin.scenario,
          origin.at_step - 1,
          forkOrigins,
        );
        if (sourceNodeId !== null) {
          edges.push({
            id: `session-fork:${origin.scenario}:${origin.at_step - 1}→${name}:${renderFrom}`,
            source: sourceNodeId,
            target: nodeId(name, renderFrom),
            _scenario: name,
            _forkEdge: true,
          });
        }
      }
    }
  }

  // Second pass: for each node, compute the list of scenarios whose path
  // passes through it. The owning scenario always counts; descendants that
  // inherit past this step also count.
  const passesThrough = new Map<string, Set<string>>();
  for (const n of nodes) passesThrough.set(n.id, new Set([n._scenario]));

  for (const name of scenarioNames) {
    const path = fullPath(name, scenarios, forkOrigins);
    for (const nid of path) {
      const set = passesThrough.get(nid);
      if (set) set.add(name);
    }
  }

  for (const n of nodes) {
    n._scenariosPassingThrough = Array.from(passesThrough.get(n.id) ?? [n._scenario]);
  }

  return { nodes, edges };
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function nodeId(scenario: string, stepIndex: number): string {
  return `session:${scenario}:step:${stepIndex}`;
}

/** Resolve the node id that OWNS a given (scenario, stepIndex) pair, walking
 *  ancestry when the scenario doesn't render that step itself (because it
 *  was inherited). Returns null if the chain bottoms out without an owner —
 *  e.g. the origin was deleted. The 64-iter guard is a safety net against
 *  pathological fork graphs; realistic chains are shallow (< 10). */
function resolveOriginNodeId(
  scenario: string,
  stepIndex: number,
  forkOrigins: Map<string, ForkOrigin>,
): string | null {
  let current: string | undefined = scenario;
  let guard = 0;
  while (current && guard++ < 64) {
    const origin = forkOrigins.get(current);
    if (!origin || stepIndex >= origin.at_step) {
      return nodeId(current, stepIndex);
    }
    current = origin.scenario;
  }
  return null;
}

/** Full path of a scenario = inherited prefix (chased through ancestry) +
 *  own rendered nodes. Used to compute passesThrough membership. */
function fullPath(
  name: string,
  scenarios: Map<string, SessionStep[]>,
  forkOrigins: Map<string, ForkOrigin>,
): string[] {
  const origin = forkOrigins.get(name);
  const ownStart = origin?.at_step ?? 0;
  const path: string[] = [];

  for (let i = 0; i < ownStart; i++) {
    const owner = resolveOriginNodeId(name, i, forkOrigins);
    if (owner !== null) path.push(owner);
  }
  const steps = scenarios.get(name);
  if (steps) {
    for (let i = ownStart; i < steps.length; i++) {
      path.push(nodeId(name, i));
    }
  }
  return path;
}
