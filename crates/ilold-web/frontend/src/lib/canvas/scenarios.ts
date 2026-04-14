import type { SessionStep, AccessLevel } from '$lib/api/types';

// ── Longest-common-prefix length (by step.function name) ────────────────────
// Returns length of the longest common prefix across all step arrays.
// Empty input → 0; single array → its full length.
export function lcpLen(arrs: SessionStep[][]): number {
  if (arrs.length === 0) return 0;
  if (arrs.length === 1) return arrs[0].length;
  const min = Math.min(...arrs.map((a) => a.length));
  let i = 0;
  while (i < min) {
    const name = arrs[0][i].function;
    if (!arrs.every((a) => a[i].function === name)) return i;
    i++;
  }
  return i;
}

// ── Composed tree types ─────────────────────────────────────────────────────

export interface ComposedNode {
  id: string;
  stepIndex: number;
  function: string;
  access: AccessLevel;
  _sessionStep: true;
  /** undefined for shared-prefix nodes; scenario name for divergent tail nodes. */
  _scenario?: string;
  /** Only on the last shared node when >1 scenarios diverge past the prefix. */
  _divergenceCount?: number;
}

export interface ComposedEdge {
  id: string;
  source: string;
  target: string;
  _scenario?: string;
}

export interface ComposedTree {
  nodes: ComposedNode[];
  edges: ComposedEdge[];
}

// ── composeScenarioTree ──────────────────────────────────────────────────────
// Pure function: takes the scenario map and returns a unified node/edge tree.
//
// Rules (design §8.1, spec §S7.3/S8.1):
// - Single scenario → no "shared" layer; every node is scenario-specific.
// - Multiple scenarios → emit shared prefix (LCP) as `session:shared:step:N`,
//   then each divergent tail as `session:<name>:step:N`.
// - When >1 scenarios diverge past the shared prefix AND a shared prefix
//   exists, the last shared node carries `_divergenceCount`.
export function composeScenarioTree(
  scenarios: Map<string, SessionStep[]>,
): ComposedTree {
  const names = Array.from(scenarios.keys());
  const arrs = names.map((n) => scenarios.get(n)!);
  const nodes: ComposedNode[] = [];
  const edges: ComposedEdge[] = [];

  if (names.length === 0) return { nodes, edges };

  // Single scenario: no shared layer — everything is scenario-specific.
  const P = names.length === 1 ? 0 : lcpLen(arrs);

  // Shared prefix nodes + edges (only when multi-scenario and prefix > 0)
  if (names.length > 1 && P > 0) {
    for (let i = 0; i < P; i++) {
      const step = arrs[0][i];
      nodes.push({
        id: `session:shared:step:${i}`,
        stepIndex: i,
        function: step.function,
        access: step.access,
        _sessionStep: true,
      });
      if (i > 0) {
        edges.push({
          id: `session-path:shared:${i - 1}→${i}`,
          source: `session:shared:step:${i - 1}`,
          target: `session:shared:step:${i}`,
        });
      }
    }
  }

  // Divergence count: how many scenarios have steps past the shared prefix.
  const divergentCount = arrs.filter((a) => a.length > P).length;
  if (divergentCount > 1 && P > 0 && nodes.length > 0) {
    nodes[nodes.length - 1]._divergenceCount = divergentCount;
  }

  // Per-scenario divergent tails.
  for (let s = 0; s < names.length; s++) {
    const name = names[s];
    const arr = arrs[s];
    if (arr.length <= P) continue;
    for (let i = P; i < arr.length; i++) {
      const step = arr[i];
      const nodeId = `session:${name}:step:${i}`;
      nodes.push({
        id: nodeId,
        stepIndex: i,
        function: step.function,
        access: step.access,
        _sessionStep: true,
        _scenario: name,
      });
      const sourceId =
        i === P
          ? P > 0
            ? `session:shared:step:${P - 1}`
            : null
          : `session:${name}:step:${i - 1}`;
      if (sourceId) {
        edges.push({
          id: `session-path:${name}:${i - 1}→${i}`,
          source: sourceId,
          target: nodeId,
          _scenario: name,
        });
      }
    }
  }

  return { nodes, edges };
}

// ── Manual test cases (reference only; no vitest wired yet — see README §Testing)
// composeScenarioTree(new Map([['main', []]]))
//   → { nodes: [], edges: [] }
// composeScenarioTree(new Map([['main', [{function:'a',access:'Public',step_index:0}]]]))
//   → 1 scenario-specific node `session:main:step:0` (no shared layer)
// composeScenarioTree(new Map([
//   ['main', [{function:'a',...},{function:'b',...}]],
//   ['alt',  [{function:'a',...},{function:'c',...}]],
// ]))
//   → shared: [a] with _divergenceCount=2; divergent: main:1 (b), alt:1 (c)
// composeScenarioTree(new Map([
//   ['main', [{function:'x',...}]],
//   ['alt',  [{function:'y',...}]],
// ]))
//   → no shared nodes; two disjoint roots main:0, alt:0
// composeScenarioTree(new Map([
//   ['s1', [{function:'a',...}]],
//   ['s2', [{function:'a',...},{function:'b',...}]],
//   ['s3', [{function:'a',...},{function:'b',...},{function:'c',...}]],
// ]))
//   → shared [a] with _divergenceCount=2 (s2 and s3 diverge past a; s1 ends at P)
