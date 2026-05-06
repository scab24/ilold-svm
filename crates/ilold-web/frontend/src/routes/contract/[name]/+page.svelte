<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick, untrack } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, getSequenceAnalysis, getFunctionSource, getProjectMap, getProgram, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis, type MapContract, type MapProgram, type ProgramDetail } from '$lib/api/rest';
  import { goto } from '$app/navigation';
  import { toggleTerminal } from '$lib/stores/terminal.svelte';
  import { openInIde } from '$lib/utils/ide-links';
  import { setSearchContext, getSearchNavigate, setSearchNavigate } from '$lib/stores/search.svelte';
  import { togglePalette, setPaletteCommands, clearPaletteCommands } from '$lib/stores/palette.svelte';
  import type { Command } from '$lib/commands/registry';
  import { getHighlightedFunction, getScenarios, getActiveScenario, getForkOrigins } from '$lib/stores/session.svelte';
  import { composeScenarioTree, type ComposedNode } from '$lib/canvas/scenarios';
  import { promptScenarioName } from '$lib/scenarios/name';
  import { dispatchScenarioAction } from '$lib/scenarios/dispatch';
  import { postCommand } from '$lib/api/session';
  import Legend from '$lib/components/contract/Legend.svelte';
  import FunctionSidebar from '$lib/components/contract/FunctionSidebar.svelte';
  import InstructionSidebar from '$lib/components/contract/InstructionSidebar.svelte';
  import { composeProgramGraph } from '$lib/canvas/program';
  import TopBar from '$lib/components/contract/TopBar.svelte';
  import StatusBar from '$lib/components/contract/StatusBar.svelte';
  import ContextMenu from '$lib/components/contract/ContextMenu.svelte';
  import FunctionSourcePanel from '$lib/components/contract/FunctionSourcePanel.svelte';
  import GraphCanvasFlow from '$lib/components/contract/GraphCanvasFlow.svelte';
  import SessionSidebar from '$lib/components/session/SessionSidebar.svelte';
  import {
    getNodes, getEdges,
    setNodes, setEdges,
    addNode, addEdge,
    addNodes, addEdges,
    removeNodesById, findNode,
    findDescendants,
    clearGraph,
    type GraphNodeData,
  } from '$lib/stores/graph.svelte';
  import { runDagreLayout } from '$lib/utils/graph-helpers';
  import { MarkerType } from '@xyflow/svelte';
  import type { Node, Edge } from '@xyflow/svelte';

  let contract: ContractDetail | null = $state(null);
  let solanaProgram: ProgramDetail | null = $state(null);
  let kind: 'solidity' | 'solana' = $state('solidity');
  let solanaCanvasIxs: Set<string> = $state(new Set());
  let error: string | null = $state(null);
  let selectedNode: any = $state(null);
  let selectedPath: any = $state(null);
  let funcPaths: Record<string, any> = $state({});
  let expandedFuncs: Set<string> = $state(new Set());
  // Driven by SvelteFlow's onselectionchange — used only for the status bar
  // selection chip. Kept as a plain count (not the full node list) since no
  // other consumer needs per-node selection data at this level.
  let selectionCount: number = $state(0);
  // Default: Seq mode is the auditor-friendly view; Session mode flips the
  // sidebar click into "add step" and hides exploration nodes on the canvas
  // so only the scenarios tree is visible.
  let mode: 'cfg' | 'sequences' | 'session' = $state('sequences');
  let seqTree: any = $state(null);
  let seqAnalysis: SequenceAnalysis | null = $state(null);
  let seqExpanded: Map<string, boolean> = $state(new Map());
  let seqDirection: 'TB' | 'LR' = $state('TB');

  // Context menu: right-click on nodes
  let contextMenu: {
    x: number;
    y: number;
    nodeId: string;
    funcName: string;
    nodeType: string;
    sessionStep?: { stepIndex: number };
  } | null = $state(null);

  let canvasFuncs: Set<string> = $state(new Set()); // functions currently on canvas

  // Project map (all contracts in the workspace) — fetched once on mount so
  // the Cmd+K palette can offer cross-contract navigation without every
  // keystroke hitting the REST endpoint.
  let projectMap: MapContract[] = $state([]);

  // Inline source-viewer panel state. Null when closed. Set by the
  // ContextMenu "View source" entry.
  let sourcePanel: { func: string } | null = $state(null);

  // Session → canvas auto-paint state
  let sessionVisCount = $state(0);
  const sessionHighlight = $derived(getHighlightedFunction());

  let callgraphRaw: CytoscapeGraph | null = $state(null);
  let flowApi: { fitView: (opts?: any) => Promise<boolean> } | null = $state(null);
  let graphCanvas: GraphCanvasFlow;
  let cfgCache: Record<string, CytoscapeGraph> = {};

  /** Get the live (drag-aware) position of a node, falling back to store position */
  function liveNodePosition(nodeId: string): { x: number; y: number } | null {
    const live = graphCanvas?.getLiveNode?.(nodeId);
    if (live?.position) return live.position;
    const stored = findNode(nodeId);
    return stored?.position ?? null;
  }

  /** Walk up _seqParent chain until we find the root function node */
  function findSeqRootFunction(nodeId: string): Node<GraphNodeData> | null {
    const visited = new Set<string>();
    let current = findNode(nodeId);
    while (current && !visited.has(current.id)) {
      visited.add(current.id);
      if (current.data._type === 'function') return current;
      const parentId = (current.data as any)._seqParent;
      if (!parentId) return null;
      current = findNode(parentId);
    }
    return null;
  }

  // BFS tree layout constants for seq subtrees (shared by relayoutSeqTree)
  const SEQ_NODE_W = 220;
  const SEQ_NODE_H = 80;
  const SEQ_SIBLING_GAP = 30; // gap between siblings at the same level
  const SEQ_LEVEL_GAP = 120;  // gap between parent and children rank

  /**
   * Re-layout a single seq subtree rooted at `rootId` (a function node).
   * - Anchors the root to its live (drag-aware) position so user drags are preserved.
   * - BFS from root via seq-edges; siblings are distributed perpendicular to seqDirection.
   * - Updates positions of all seq-next nodes in the subtree.
   * - Updates sourceHandle/targetHandle on all seq-edges in the subtree to match seqDirection.
   *
   * Callers MUST add any new nodes/edges to the store BEFORE invoking this helper,
   * so the BFS walk includes them. Placeholder positions on new nodes are fine.
   */
  function relayoutSeqTree(rootId: string) {
    const root = findNode(rootId);
    if (!root) return;
    const rootPos = liveNodePosition(rootId) ?? root.position;

    // 1. Collect the full seq subtree (root + all transitively-linked seq-next nodes)
    const subtreeIds = new Set<string>([rootId]);
    let added = true;
    while (added) {
      added = false;
      for (const n of getNodes()) {
        if (n.data._type === 'seq-next' && !subtreeIds.has(n.id)) {
          const sp = (n.data as any)._seqParent as string;
          if (sp && subtreeIds.has(sp)) {
            subtreeIds.add(n.id);
            added = true;
          }
        }
      }
    }

    // 2. Build children index from seq-edges restricted to the subtree
    const childrenMap = new Map<string, string[]>();
    const subtreeEdgeIds = new Set<string>();
    for (const e of getEdges()) {
      if (e.data?._type === 'seq-edge' && subtreeIds.has(e.source) && subtreeIds.has(e.target)) {
        const arr = childrenMap.get(e.source) ?? [];
        arr.push(e.target);
        childrenMap.set(e.source, arr);
        subtreeEdgeIds.add(e.id);
      }
    }

    // 3. BFS from root, assigning levels
    const levels = new Map<string, number>();
    levels.set(rootId, 0);
    const queue = [rootId];
    let maxLevel = 0;
    while (queue.length > 0) {
      const id = queue.shift()!;
      const lvl = levels.get(id)!;
      for (const kid of childrenMap.get(id) ?? []) {
        if (!levels.has(kid)) {
          levels.set(kid, lvl + 1);
          maxLevel = Math.max(maxLevel, lvl + 1);
          queue.push(kid);
        }
      }
    }

    // 4. Group nodes by level and compute positions anchored at rootPos
    const byLevel: string[][] = Array.from({ length: maxLevel + 1 }, () => []);
    for (const [id, lvl] of levels) byLevel[lvl].push(id);

    const isLR = seqDirection === 'LR';
    const posMap = new Map<string, { x: number; y: number }>();
    for (let lvl = 1; lvl <= maxLevel; lvl++) {
      const ids = byLevel[lvl];
      const count = ids.length;
      if (isLR) {
        // Children to the right, stacked vertically at same X
        const totalH = count * SEQ_NODE_H + (count - 1) * SEQ_SIBLING_GAP;
        const startY = rootPos.y + SEQ_NODE_H / 2 - totalH / 2;
        const x = rootPos.x + lvl * (SEQ_NODE_W + SEQ_LEVEL_GAP);
        ids.forEach((id, i) => {
          posMap.set(id, { x, y: startY + i * (SEQ_NODE_H + SEQ_SIBLING_GAP) });
        });
      } else {
        // Children below, in a horizontal row at same Y
        const totalW = count * SEQ_NODE_W + (count - 1) * SEQ_SIBLING_GAP;
        const startX = rootPos.x + SEQ_NODE_W / 2 - totalW / 2;
        const y = rootPos.y + lvl * (SEQ_NODE_H + SEQ_LEVEL_GAP);
        ids.forEach((id, i) => {
          posMap.set(id, { x: startX + i * (SEQ_NODE_W + SEQ_SIBLING_GAP), y });
        });
      }
    }

    // 5. Apply positions to seq-next nodes in this subtree
    setNodes(getNodes().map(n => {
      if (n.data._type === 'seq-next' && posMap.has(n.id)) {
        return { ...n, position: posMap.get(n.id)! };
      }
      return n;
    }));

    // 6. Update handle orientation on seq-edges in this subtree to match seqDirection
    const sh = isLR ? 'r' : 'b';
    const th = isLR ? 'l' : 't';
    setEdges(getEdges().map(e => {
      if (subtreeEdgeIds.has(e.id)) {
        return { ...e, sourceHandle: sh, targetHandle: th };
      }
      return e;
    }));
  }

  /** Re-layout and re-orient all expanded seq subtrees (used when seqDirection changes) */
  function reorientAllSeqSubtrees() {
    // Find all root functions that have seq-next children
    const roots = new Set<string>();
    for (const n of getNodes()) {
      if (n.data._type === 'seq-next') {
        const root = findSeqRootFunction(n.id);
        if (root) roots.add(root.id);
      }
    }
    for (const rootId of roots) {
      relayoutSeqTree(rootId);
    }
  }

  /** Merge an opacity value into an edge's style string */
  function edgeStyle(base: string | undefined, opacity: number): string {
    // Remove existing opacity from base style, then append new one
    const cleaned = (base ?? '').replace(/opacity:\s*[\d.]+;?/g, '').trim();
    const sep = cleaned && !cleaned.endsWith(';') ? '; ' : ' ';
    return `${cleaned}${cleaned ? sep : ''}opacity: ${opacity}`.trim();
  }

  /** Reset all _dimmed state on nodes and edges */
  function resetAllDimmed() {
    setNodes(getNodes().map(n => {
      if ('_dimmed' in n.data && n.data._dimmed) {
        return { ...n, data: { ...n.data, _dimmed: false } as GraphNodeData };
      }
      return n;
    }));
    setEdges(getEdges().map(e => {
      if (e.data?._dimmed) {
        return { ...e, style: edgeStyle(e.style, 1), data: { ...e.data, _dimmed: false } };
      }
      return e;
    }));
  }

  /** Remove every seq-next descendant of a seq node. */
  function collapseAllDescendants(nodeId: string) {
    const allDesc = findDescendants(nodeId);
    const toRemove = new Set<string>();
    for (const id of allDesc) {
      const n = findNode(id);
      if (n && n.data._type === 'seq-next') toRemove.add(id);
    }
    if (toRemove.size > 0) removeNodesById(toRemove);
  }

  /** Dim all function nodes (except excludeId) and call edges to 0.1 opacity */
  function dimFunctionLayer(excludeId?: string) {
    setNodes(getNodes().map(n => {
      if (n.data._type === 'function' && n.id !== excludeId) {
        return { ...n, data: { ...n.data, _dimmed: true } as GraphNodeData };
      }
      return n;
    }));
    setEdges(getEdges().map(e => {
      if (e.data?._type === 'call') {
        return { ...e, style: edgeStyle(e.style, 0.1), data: { ...e.data, _dimmed: true } };
      }
      return e;
    }));
  }

  /** Merge stroke-dasharray + stroke into edge style string */
  function dashedEdgeStyle(stroke: string): string {
    return `stroke-dasharray: 5 3; stroke: ${stroke}`;
  }

  /** Extract condition text from Debug-format kind string */
  function extractCondition(kind: string): string | undefined {
    const match = kind.match(/condition:\s*"([^"]+)"/);
    return match?.[1];
  }

  /** Extract catch kind from CatchClause Debug-format string */
  function extractCatchKind(kind: string): string | undefined {
    const match = kind.match(/kind:\s*"([^"]+)"/);
    return match?.[1];
  }

  /** Truncate label to ~30 chars */
  function truncateLabel(text: string, max = 30): string {
    return text.length > max ? text.slice(0, max - 1) + '…' : text;
  }

  /** Compute edge visual props from CFG edge kind */
  function cfgEdgeStyle(kind: string): {
    color: string;
    label?: string;
    animated: boolean;
    sourceHandle: string;
    targetHandle: string;
  } {
    if (kind.startsWith('ConditionalTrue')) {
      const cond = extractCondition(kind);
      return {
        color: 'var(--color-success)',
        label: cond ? truncateLabel(cond) : '✓',
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    if (kind.startsWith('ConditionalFalse')) {
      const cond = extractCondition(kind);
      return {
        color: 'var(--color-warning)',
        label: cond ? truncateLabel(cond) : '✗',
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    if (kind === 'LoopBack') {
      return {
        color: 'var(--color-accent)',
        animated: true,
        sourceHandle: 'r',
        targetHandle: 'r',
      };
    }
    if (kind === 'LoopExit') {
      return {
        color: 'var(--color-text-muted)',
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    if (kind === 'ExternalCallSuccess') {
      return {
        color: 'var(--color-success-light)',
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    if (kind === 'ExternalCallFailure') {
      return {
        color: 'var(--color-danger)',
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    if (kind.startsWith('CatchClause')) {
      const catchKind = extractCatchKind(kind);
      return {
        color: 'var(--color-danger-light)',
        label: catchKind ? truncateLabel(catchKind) : undefined,
        animated: false,
        sourceHandle: 'b',
        targetHandle: 't',
      };
    }
    // Unconditional / fallback
    return {
      color: 'var(--color-text-dim)',
      animated: false,
      sourceHandle: 'b',
      targetHandle: 't',
    };
  }

  // ── Session → canvas auto-paint (Phase S5) ──────────────────
  // The session store owns an `activeScenario` + Map<name, steps[]>. This
  // effect composes ALL scenarios into a unified tree (shared prefix +
  // divergent tails) via `composeScenarioTree`, then syncs the canvas.
  //
  // Strategy: on every run, remove all `session:*` step nodes/edges and
  // re-emit from the composed tree. Cheap (nodes are tiny) and avoids a
  // fragile per-id diff. `activeScenario` is read so restyling (pill colors,
  // active glow vs muted) re-runs when the user switches scenarios even if
  // the tree shape is identical.
  $effect(() => {
    // Reactive reads — trigger re-run on scenario changes + active-scenario flip.
    const scenarios = getScenarios();
    const forkOrigins = getForkOrigins();
    const active = getActiveScenario();
    if (!contract || !callgraphRaw) return;

    const tree = composeScenarioTree(scenarios, forkOrigins);

    // Graph-store reads/writes are wrapped in untrack() to prevent a reactive
    // cycle: reading getNodes() would subscribe this effect to `nodes`, and
    // the subsequent removeNodesById/addNodes/addEdges would re-trigger it
    // (infinite loop that froze the canvas on every c <func>).
    untrack(() => {
      const toRemove = new Set<string>();
      for (const n of getNodes()) {
        if (n.id.startsWith('session:')) toRemove.add(n.id);
      }
      if (toRemove.size > 0) removeNodesById(toRemove);

      if (tree.nodes.length === 0) {
        sessionVisCount = 0;
        return;
      }

      const allFuncs = [...(contract?.functions ?? []), ...(contract?.inherited_functions ?? [])];

      // Lane-per-scenario tree. Each scenario renders only its divergent
      // tail (`steps[at_step..end]`) on its own horizontal lane; the
      // inherited prefix is reused from the origin's lane. A fork edge
      // connects origin:step:{at_step-1} → self:step:{at_step} so branches
      // visibly emerge from their fork point.
      const SESSION_BASE_X = 200;
      const SESSION_BASE_Y = 300;
      const SESSION_STEP_WIDTH = 280;
      const SESSION_LANE_HEIGHT = 100;

      const composedNodes: Node<GraphNodeData>[] = tree.nodes.map((cn: ComposedNode) => {
        const funcDetail = allFuncs.find((f: any) => f.name === cn.function);
        return {
          id: cn.id,
          type: 'function',
          position: {
            x: SESSION_BASE_X + cn.stepIndex * SESSION_STEP_WIDTH,
            y: SESSION_BASE_Y + cn.lane * SESSION_LANE_HEIGHT,
          },
          data: {
            _type: 'function',
            _sessionStep: true,
            _scenario: cn._scenario,
            _scenariosPassingThrough: cn._scenariosPassingThrough,
            _activeScenario: active,
            stepIndex: cn.stepIndex,
            label: cn.function,
            is_external: false,
            contractName: contract!.name,
            visibility: funcDetail?.visibility,
            mutability: funcDetail?.mutability,
            path_count: funcDetail?.path_count,
            modifiers: funcDetail?.modifiers,
          } as GraphNodeData,
        } as Node<GraphNodeData>;
      });
      addNodes(composedNodes);

      const nodeScenarios = new Map<string, string[]>(
        tree.nodes.map((n) => [n.id, n._scenariosPassingThrough]),
      );

      const composedEdges: Edge[] = tree.edges.map((ce) => {
        const isFork = ce._forkEdge === true;
        const sourceScns = nodeScenarios.get(ce.source) ?? [];
        const targetScns = nodeScenarios.get(ce.target) ?? [];
        const onActivePath = sourceScns.includes(active) && targetScns.includes(active);

        const color = onActivePath
          ? (isFork ? 'var(--color-accent)' : 'var(--color-accent-hover)')
          : 'var(--color-text-dim)';
        const opacity = onActivePath ? 1 : 0.4;

        return {
          id: ce.id,
          source: ce.source,
          target: ce.target,
          sourceHandle: 'r',
          targetHandle: 'l',
          type: 'default',
          animated: isFork && onActivePath,
          style: `stroke: ${color}; opacity: ${opacity}; ${isFork ? 'stroke-dasharray: 4 4;' : ''}`,
          markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color },
          labelBgStyle: { fill: 'var(--color-surface)', fillOpacity: 0.85 },
          labelBgPadding: [3, 5] as [number, number],
          data: { _type: 'session-path', _scenario: ce._scenario, _forkEdge: isFork },
        };
      });
      addEdges(composedEdges);

      sessionVisCount = tree.nodes.length;
    });
  });

  // Session mode visibility filter: hide every non-session node and every
  // non-session-path edge so the canvas shows the scenarios tree alone.
  // Switching mode back unhides — no state is destroyed.
  $effect(() => {
    const currentMode = mode;
    // Wrapped in untrack: setNodes/setEdges would otherwise re-trigger this
    // effect via getNodes()/getEdges() subscriptions.
    untrack(() => {
      setNodes(getNodes().map((n) => {
        const isSessionNode = (n.data as any)._sessionStep === true;
        const shouldHide = currentMode === 'session' && !isSessionNode;
        return n.hidden === shouldHide ? n : { ...n, hidden: shouldHide };
      }));
      setEdges(getEdges().map((e) => {
        const isSessionEdge = (e.data as any)?._type === 'session-path';
        const shouldHide = currentMode === 'session' && !isSessionEdge;
        return e.hidden === shouldHide ? e : { ...e, hidden: shouldHide };
      }));
    });
  });

  // Invalidate stale NodeInspector selection. Any flow that removes
  // nodes (CFG collapse, removeFuncFromCanvas, removeSeqNode, DEL key,
  // Clear from the sidebar) converges here — callers don't need to
  // remember to null out selectedNode themselves. The read inside
  // findNode creates a reactive dependency so the guard re-runs whenever
  // the graph store mutates. Safe against loops: when we set
  // selectedNode = null the effect re-runs, short-circuits on the null
  // check, and exits without further writes.
  $effect(() => {
    if (!selectedNode) return;
    if (!findNode(selectedNode.id)) {
      selectedNode = null;
      selectedPath = null;
    }
  });

  // Highlight the function node when the session broadcasts session_highlight
  $effect(() => {
    const funcName = sessionHighlight;
    if (!funcName) return;
    const node = getNodes().find(
      n => n.data._type === 'function' && n.data.label === funcName
    );
    if (node) selectedNode = node;
  });

  function handleSolanaIxAdd(ix: string) {
    solanaCanvasIxs = new Set([...solanaCanvasIxs, ix]);
    const node = findNode(`ix:${ix}`);
    if (node && flowApi) {
      flowApi.fitView({ nodes: [{ id: node.id }], padding: 0.5, duration: 400 });
    }
  }

  function handleSolanaIxRemove(ix: string) {
    const next = new Set(solanaCanvasIxs);
    next.delete(ix);
    solanaCanvasIxs = next;
    removeNodesById([`ix:${ix}`]);
  }

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    // Graph store is global — stale nodes from a previous contract must be
    // wiped or they'd pollute this contract's canvas (and leave the sidebar
    // out of sync because local Sets re-init empty on re-mount).
    clearGraph();
    canvasFuncs = new Set();
    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    setSearchContext(contractName);
    try {
      const pm = await getProjectMap();
      kind = pm.kind === 'solana' ? 'solana' : 'solidity';
      if (kind === 'solana') {
        try {
          solanaProgram = await getProgram(contractName);
        } catch {
          error = `Program "${contractName}" not found`;
          return;
        }
        projectMap = [];
        const composed = composeProgramGraph(solanaProgram);
        setNodes(composed.nodes);
        setEdges(composed.edges);
        solanaCanvasIxs = new Set(solanaProgram.instructions.map((i) => i.name));
        return;
      }
      projectMap = pm.contracts ?? [];
      contract = await getContract(contractName);
      const callgraphData = await getCallGraph(contractName);
      callgraphRaw = callgraphData;
      try { seqTree = await getSequences(contractName); } catch {}
      try { seqAnalysis = await getSequenceAnalysis(contractName); } catch {}
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  // Listen for search result navigation. Only `getSearchNavigate()` is
  // tracked — everything else is accessed via untrack so mutations inside
  // the IIFE (canvasFuncs, funcPaths, expandedFuncs, edges via
  // highlightPath) don't re-enter this effect and trigger an
  // effect_update_depth_exceeded loop. The effect re-runs exactly when
  // the palette publishes a new navigation target; subsequent state
  // writes are handled inside the async task.
  $effect(() => {
    const nav = getSearchNavigate();
    if (!nav) return;
    untrack(() => {
      if (!contract) return;
      if (nav.contract !== contract.name) return;

      let stale = false;

      (async () => {
        try {
          if (!canvasFuncs.has(nav.func)) {
            addFuncToCanvas(nav.func);
            await tick();
          }
          if (stale || !contract) return;

          if (!funcPaths[nav.func]) {
            funcPaths[nav.func] = await getPaths(contract.name, nav.func);
            funcPaths = { ...funcPaths };
          }
          if (stale || !contract) return;

          if (!expandedFuncs.has(nav.func)) {
            await toggleFuncExpand(nav.func);
          }
          if (stale) return;

          const funcNode = getNodes().find(
            n => n.data._type === 'function' && n.data.label === nav.func
          );
          if (funcNode) {
            selectedNode = { ...funcNode.data, id: funcNode.id };
            const path = funcPaths[nav.func]?.paths?.find((p: any) => p.id === nav.pathId);
            if (path) highlightPath(nav.func, path);
            if (flowApi) {
              await tick();
              flowApi.fitView({ nodes: [{ id: funcNode.id }], padding: 0.5, duration: 400 });
            }
          }
        } finally {
          if (!stale) setSearchNavigate(null);
        }
      })();
    });
  });

  // Sidebar click dispatcher. In Session mode, the sidebar is the entry
  // point for building a scenario — clicking a function fires a Call
  // command and the WS session_add_node event repaints the scenario tree.
  // In CFG/Seq modes, clicking adds the function as an exploration node.
  function notifyFailure(label: string, e: unknown) {
    const reason = e instanceof Error ? e.message : String(e);
    console.warn(`${label} failed:`, e);
    alert(`${label} failed:\n\n${reason}`);
  }

  async function handleSidebarAdd(funcName: string) {
    if (mode === 'session') {
      try {
        await postCommand({ Call: { func: funcName } }, contract?.name);
      } catch (e) {
        notifyFailure('add step', e);
      }
      return;
    }
    addFuncToCanvas(funcName);
  }

  function addFuncToCanvas(funcName: string) {
    if (!callgraphRaw || canvasFuncs.has(funcName)) return;
    const nodeData = callgraphRaw.nodes.find(n => n.data.label === funcName);
    if (!nodeData) return;

    // Look up enrichment data from ContractDetail
    const allFuncs = [...(contract?.functions ?? []), ...(contract?.inherited_functions ?? [])];
    const funcDetail = allFuncs.find((f: any) => f.name === funcName);

    const count = canvasFuncs.size;
    const position = {
      x: 300 + (count % 3 - 1) * 200,
      y: 200 + Math.floor(count / 3) * 100,
    };

    addNode({
      id: nodeData.data.id,
      type: 'function',
      position,
      data: {
        _type: 'function',
        label: nodeData.data.label,
        is_external: nodeData.data.is_external ?? false,
        contractName: nodeData.data.contract,
        visibility: funcDetail?.visibility,
        mutability: funcDetail?.mutability,
        path_count: funcDetail?.path_count,
        modifiers: funcDetail?.modifiers,
      },
    } as Node<GraphNodeData>);

    // Add call edges where BOTH source and target are on canvas
    for (const e of callgraphRaw.edges) {
      const srcOnCanvas = canvasFuncs.has(
        callgraphRaw.nodes.find(n => n.data.id === e.data.source)?.data.label ?? ''
      ) || e.data.source === nodeData.data.id;
      const tgtOnCanvas = canvasFuncs.has(
        callgraphRaw.nodes.find(n => n.data.id === e.data.target)?.data.label ?? ''
      ) || e.data.target === nodeData.data.id;

      if (srcOnCanvas && tgtOnCanvas) {
        if (!getEdges().some(existing => existing.id === e.data.id)) {
          addEdge({
            id: e.data.id,
            source: e.data.source,
            target: e.data.target,
            sourceHandle: 'r',
            targetHandle: 'l',
            type: 'default',
            style: `stroke: var(--color-text-muted)`,
            markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: 'var(--color-text-muted)' },
            data: { _type: 'call', kind: e.data.kind },
            ...(e.data.call_count > 1 ? {
              label: `\u00D7${e.data.call_count}`,
              labelBgStyle: { fill: 'var(--color-surface)', fillOpacity: 0.85 },
              labelBgPadding: [3, 5] as [number, number],
            } : {}),
          });
        }
      }
    }

    canvasFuncs.add(funcName);
    canvasFuncs = new Set(canvasFuncs);
  }

  function removeSeqNode(nodeId: string) {
    const descendants = findDescendants(nodeId);
    descendants.add(nodeId);
    removeNodesById(descendants);

    seqExpanded.delete(nodeId);
    for (const k of seqExpanded.keys()) {
      if (!findNode(k)) seqExpanded.delete(k);
    }
    seqExpanded = new Map(seqExpanded);
  }

  function removeFuncFromCanvas(funcName: string) {
    if (!canvasFuncs.has(funcName)) return;
    const funcNode = getNodes().find(
      n => n.data._type === 'function' && n.data.label === funcName
    );
    if (!funcNode) return;
    const nodeId = funcNode.id;

    const toRemove = new Set<string>([nodeId]);

    // CFG children (blocks with _parentFunc === funcName)
    for (const n of getNodes()) {
      if ('_parentFunc' in n.data && n.data._parentFunc === funcName) {
        toRemove.add(n.id);
      }
    }

    // Seq descendants (recursive via _seqParent)
    const seqDesc = findDescendants(nodeId);
    for (const id of seqDesc) toRemove.add(id);

    // Also find seq nodes whose _seqParent starts with nodeId→
    for (const n of getNodes()) {
      if ('_seqParent' in n.data) {
        const sp = n.data._seqParent as string;
        if (sp === nodeId || sp.startsWith(nodeId + '→')) {
          toRemove.add(n.id);
        }
      }
    }

    removeNodesById(toRemove);

    if (expandedFuncs.has(funcName)) {
      resetAllDimmed();
    }

    canvasFuncs.delete(funcName);
    canvasFuncs = new Set(canvasFuncs);
    expandedFuncs.delete(funcName);
    expandedFuncs = new Set(expandedFuncs);
    seqExpanded.delete(nodeId);
    seqExpanded = new Map(seqExpanded);
  }

  // Keyboard-driven delete from the canvas (Figma/Excalidraw pattern).
  // Dispatches each selected node to the right existing helper so all the
  // store bookkeeping (canvasFuncs, expandedFuncs, seqExpanded, dim state)
  // stays in one place. Session mode is guarded at the canvas prop level.
  function handleNodesDelete(nodes: Node<GraphNodeData>[]) {
    if (mode === 'session') return;
    const funcsToRemove = new Set<string>();
    const seqNodesToRemove: string[] = [];
    for (const n of nodes) {
      const d = n.data;
      if (d._type === 'function') {
        funcsToRemove.add(d.label);
      } else if (d._type === 'block') {
        const parent = (d as any)._parentFunc;
        if (parent) funcsToRemove.add(parent);
      } else if (d._type === 'seq-next') {
        seqNodesToRemove.push(n.id);
      }
    }
    for (const nid of seqNodesToRemove) removeSeqNode(nid);
    for (const fname of funcsToRemove) removeFuncFromCanvas(fname);
    selectedNode = null;
  }

  // --- Event handlers ---

  async function handleNodeTap(node: Node<GraphNodeData>) {
    const data = node.data;

    if (!selectedNode || selectedNode.id !== node.id) {
      selectedPath = null;
      // Reset CFG block highlighting when clicking a different node
      setNodes(getNodes().map(n => {
        if (n.data._type === 'block' && '_dimmed' in n.data && n.data._dimmed) {
          return { ...n, data: { ...n.data, _dimmed: false } as GraphNodeData };
        }
        return n;
      }));
    }

    selectedNode = { ...data, id: node.id };
    contextMenu = null;

    const funcName = data._type === 'function' ? data.label
      : data._type === 'block' ? (data as any)._parentFunc
      : data._type === 'seq-next' ? ((data as any)._funcName || data.label)
      : null;

    if (funcName && contract && !funcPaths[funcName]) {
      try {
        funcPaths[funcName] = await getPaths(contract.name, funcName);
        funcPaths = { ...funcPaths };
      } catch {}
    }
  }

  function handleBackgroundTap() {
    selectedNode = null;
    selectedPath = null;
    contextMenu = null;
    resetAllDimmed();
  }

  // Right-click "⎇ Fork scenario here": forks the active scenario, keeping
  // steps [0..=stepIndex] (i.e. truncate at stepIndex + 1). Surfaces backend
  // errors via console.warn — ScenarioStore enforces uniqueness of names.
  async function handleForkScenario(stepIndex: number) {
    contextMenu = null;
    const name = promptScenarioName();
    if (!name) return;
    await dispatchScenarioAction(
      { Fork: { name, at_step: stepIndex + 1 } },
      contract?.name,
      'fork',
    );
  }

  // Toolbar ↶: remove the last step of the active scenario.
  async function handleSessionBack() {
    try {
      await postCommand('Back', contract?.name);
    } catch (e) {
      notifyFailure('session back', e);
    }
  }

  // Toolbar 🗑: wipe every step of the active scenario (with confirm).
  async function handleSessionClear() {
    const active = getActiveScenario();
    const steps = getScenarios().get(active) ?? [];
    if (steps.length === 0) return;
    if (!confirm(`Clear all ${steps.length} step(s) from scenario "${active}"?`)) return;
    try {
      await postCommand('Clear', contract?.name);
    } catch (e) {
      notifyFailure('session clear', e);
    }
  }

  // Right-click "{} View source": open the inline CodeMirror panel.
  function handleViewSource(funcName: string) {
    contextMenu = null;
    sourcePanel = { func: funcName };
  }

  // Right-click "↗ Open in code": fetch the function's absolute path + line
  // from the source endpoint and fire the `vscode://` deep link. If no IDE
  // is registered the browser silently drops the request — no UI nag.
  async function handleOpenInIde(funcName: string) {
    contextMenu = null;
    if (!contract) return;
    try {
      const res = await getFunctionSource(contract.name, funcName);
      openInIde(res.file_path, res.span.start_line, res.span.start_col);
    } catch (e) {
      notifyFailure('open in IDE', e);
    }
  }

  // Right-click "✕ Remove from here": truncate the active scenario at
  // `stepIndex` by firing N Back commands. N = current length - stepIndex.
  // Using Back (which the backend already supports) avoids needing a new
  // truncate command on the server.
  async function handleRemoveFromHere(stepIndex: number) {
    contextMenu = null;
    const active = getActiveScenario();
    const steps = getScenarios().get(active) ?? [];
    const backCount = steps.length - stepIndex;
    if (backCount <= 0) return;
    for (let i = 0; i < backCount; i++) {
      try {
        await postCommand('Back', contract?.name);
      } catch (e) {
        notifyFailure('remove-from-here iteration', e);
        break;
      }
    }
  }

  function handleContextMenu(event: MouseEvent, node: Node<GraphNodeData>) {
    const data = node.data;
    // "Fork scenario here" is only meaningful when the active scenario's
    // path passes through this node — either it owns the node or inherits
    // it from an ancestor. `_scenariosPassingThrough` already encodes both
    // cases so the check is a single Array.includes.
    let sessionStep: { stepIndex: number } | undefined;
    if (data._type === 'function' && data._sessionStep === true) {
      const { _scenariosPassingThrough: scns, _activeScenario: active, stepIndex: idx } = data;
      if (typeof idx === 'number' && active && scns?.includes(active)) {
        sessionStep = { stepIndex: idx };
      }
    }
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      nodeId: node.id,
      funcName: data._type === 'function' ? data.label : ('_parentFunc' in data ? (data as any)._parentFunc : ('_funcName' in data ? (data as any)._funcName : (data as any).label)),
      nodeType: data._type,
      sessionStep,
    };
  }

  async function handleNodeClick(node: Node<GraphNodeData>, event?: MouseEvent) {
    // Selection first (sync), then expand/collapse (async)
    handleNodeTap(node);
    // In Session mode the canvas is read-only for exploration — clicks only
    // select. Expansion would add CFG blocks / seq-next children that we
    // deliberately hide in this mode.
    if (mode === 'session') return;
    const d = node.data;
    if (d._type === 'function' && !d.is_external) {
      await handleFunctionTap(d.label, node.id);
    } else if (d._type === 'seq-next') {
      await handleSeqNodeTap((d as any)._funcName || d.label, node.id, (d as any)._seqParent, event);
    }
  }

  async function handleFunctionTap(funcName: string, nodeId: string) {
    if (mode === 'cfg') {
      await toggleFuncExpand(funcName);
    } else if (mode === 'sequences') {
      await toggleSeqExpand(funcName, nodeId);
    }
  }

  async function handleSeqNodeTap(funcName: string, nodeId: string, seqParent: string, event?: MouseEvent) {
    // Plain click commits to one sibling path: collapse the auto-expanded
    // sub-trees of all siblings at this level. Shift+click skips the
    // collapse so the user can keep multiple branches open in parallel —
    // matches the "Shift+click → add branch" hint shown in Legend.
    const keepSiblings = event?.shiftKey === true;
    if (seqParent && !keepSiblings) {
      const siblings = getNodes().filter(
        n => n.data._type === 'seq-next'
          && (n.data as any)._seqParent === seqParent
          && n.id !== nodeId
      );
      const toRemove = new Set<string>();
      for (const sib of siblings) {
        for (const id of findDescendants(sib.id)) toRemove.add(id);
        toRemove.add(sib.id);
        seqExpanded.delete(sib.id);
      }
      if (toRemove.size > 0) removeNodesById(toRemove);
      seqExpanded = new Map(seqExpanded);
    }

    await toggleSeqExpand(funcName, nodeId);
  }

  async function toggleFuncExpand(funcName: string, anchorNodeId?: string) {
    if (!contract) return;
    const parentId = anchorNodeId || `${contract.name}::${funcName}`;

    if (expandedFuncs.has(funcName)) {
      // --- COLLAPSE ---
      const toRemove = new Set<string>();
      for (const n of getNodes()) {
        if ('_parentFunc' in n.data && n.data._parentFunc === funcName) {
          toRemove.add(n.id);
        }
      }
      removeNodesById(toRemove);
      resetAllDimmed();

      expandedFuncs.delete(funcName);
      expandedFuncs = new Set(expandedFuncs);
      return;
    }

    // --- EXPAND ---
    if (!cfgCache[funcName]) {
      cfgCache[funcName] = await getCfg(contract.name, funcName);
    }
    const cfg = cfgCache[funcName];
    const parentPos = liveNodePosition(parentId) ?? { x: 300, y: 200 };

    // 1. Build Svelte Flow nodes (initially at parent position for animation)
    const cfgNodes: Node<GraphNodeData>[] = cfg.nodes.map(n => ({
      id: `cfg:${funcName}:${n.data.id}`,
      type: 'block',
      position: { ...parentPos },
      data: {
        _type: 'block' as const,
        label: n.data.label,
        node_type: n.data.node_type,
        _parentFunc: funcName,
        statements: n.data.statements,
      },
    }));

    // 2. Build edges with color-coded styles, arrows, and explicit handles
    const cfgEdges: Edge[] = cfg.edges.map((e, i) => {
      const es = cfgEdgeStyle(e.data.kind);
      return {
        id: `cfg-edge:${funcName}:${i}`,
        source: `cfg:${funcName}:${e.data.source}`,
        target: `cfg:${funcName}:${e.data.target}`,
        sourceHandle: es.sourceHandle,
        targetHandle: es.targetHandle,
        type: 'smoothstep',
        data: {
          _type: 'cfg-edge',
          _parentFunc: funcName,
          kind: e.data.kind,
        },
        label: es.label,
        labelBgStyle: es.label ? { fill: 'var(--color-surface)', fillOpacity: 0.85 } : undefined,
        labelBgPadding: es.label ? [4, 6] as [number, number] : undefined,
        style: `stroke: ${es.color}`,
        animated: es.animated,
        markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16, color: es.color },
      };
    });

    // 3. Link edge: function node → CFG entry block
    const entryNode = cfg.nodes.find(n => n.data.node_type === 'Entry');
    if (entryNode) {
      cfgEdges.push({
        id: `cfg-link:${funcName}`,
        source: parentId,
        target: `cfg:${funcName}:${entryNode.data.id}`,
        sourceHandle: 'b',
        targetHandle: 't',
        type: 'smoothstep',
        data: { _type: 'cfg-edge', _parentFunc: funcName, kind: 'expand' },
        style: dashedEdgeStyle('var(--color-accent-dark)'),
        markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16, color: 'var(--color-accent-dark)' },
      });
    }

    // 4. Run dagre on CFG subset to get positions
    const layoutNodes = runDagreLayout(cfgNodes, cfgEdges, {
      rankDir: 'TB', nodeSep: 40, rankSep: 60, nodeWidth: 180,
    });

    // 5. Offset all positions below the parent function node
    let minX = Infinity, minY = Infinity, maxX = -Infinity;
    for (const n of layoutNodes) {
      if (n.position.x < minX) minX = n.position.x;
      if (n.position.x > maxX) maxX = n.position.x;
      if (n.position.y < minY) minY = n.position.y;
    }
    const centerX = (minX + maxX) / 2;
    const offsetX = parentPos.x - centerX;
    const offsetY = parentPos.y + 60 - minY;

    const finalPositions = new Map<string, { x: number; y: number }>();
    for (const n of layoutNodes) {
      finalPositions.set(n.id, { x: n.position.x + offsetX, y: n.position.y + offsetY });
    }

    // Add nodes at their final dagre-computed positions (no animation, predictable)
    for (const n of cfgNodes) {
      const final = finalPositions.get(n.id);
      if (final) n.position = final;
    }
    addNodes(cfgNodes);
    addEdges(cfgEdges);

    // Dim function nodes + call edges
    dimFunctionLayer(parentId);

    expandedFuncs.add(funcName);
    expandedFuncs = new Set(expandedFuncs);
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    // ── COLLAPSE ──
    if (seqExpanded.has(parentNodeId)) {
      collapseAllDescendants(parentNodeId);
      seqExpanded.delete(parentNodeId);
      seqExpanded = new Map(seqExpanded);

      // If no seq-next nodes remain, un-dim everything
      const anySeq = getNodes().some(n => n.data._type === 'seq-next');
      if (!anySeq) resetAllDimmed();
      return;
    }

    // ── EXPAND ──
    if (!seqTree || !seqTree.functions) return;

    // Find the root function node for this seq subtree (walk up _seqParent chain)
    const rootFunc = findSeqRootFunction(parentNodeId);
    if (!rootFunc) return;

    const seqFunctions: Array<{ name: string; visibility: string; read_only: boolean; path_count: number }> = seqTree.functions;

    // Show every contract function as a candidate next-step, matching the
    // CLI `f` listing. The "interesting transition" signal (⚠ conditions
    // badge + dashed border) is preserved automatically via the per-child
    // `_transition` lookup below — no filtering needed here.
    const targets = seqFunctions;

    // Reuse the same lookup the scenarios canvas uses to pull modifier/
    // mutability info that isn't on `SequenceFunction`. `contract` already
    // holds the full function detail.
    const allFuncs = [...(contract?.functions ?? []), ...(contract?.inherited_functions ?? [])];

    // Build new seq-next children with placeholder positions — relayoutSeqTree
    // will assign final positions from the shared BFS walk.
    const newNodes: Node<GraphNodeData>[] = [];
    const newEdges: Edge[] = [];
    for (const func of targets) {
      const targetName = func.name;
      const nodeId = `seq:${parentNodeId}→${targetName}`;
      const transition = seqAnalysis?.transitions?.find(
        t => t.from === funcName && t.to === targetName
      ) ?? null;
      const funcDetail = allFuncs.find((f: any) => f.name === targetName);

      newNodes.push({
        id: nodeId,
        type: 'sequence',
        position: { x: 0, y: 0 },
        data: {
          _type: 'seq-next',
          label: targetName,
          _funcName: targetName,
          _seqParent: parentNodeId,
          readOnly: func.read_only,
          pathCount: func.path_count,
          visibility: func.visibility,
          modifiers: funcDetail?.modifiers,
          _transition: transition,
        },
      } as Node<GraphNodeData>);

      newEdges.push({
        id: `seq-edge:${parentNodeId}→${targetName}`,
        source: parentNodeId,
        sourceHandle: seqDirection === 'LR' ? 'r' : 'b',
        target: nodeId,
        targetHandle: seqDirection === 'LR' ? 'l' : 't',
        type: 'default',
        data: { _type: 'seq-edge' },
        style: transition?.shared_state?.length
          ? dashedEdgeStyle('var(--color-warning)')
          : undefined,
      });
    }

    // Commit new nodes/edges to the store first, then let the shared helper
    // re-run BFS over the whole subtree (root + existing + new) coherently.
    addNodes(newNodes);
    addEdges(newEdges);
    relayoutSeqTree(rootFunc.id);

    dimFunctionLayer(rootFunc.id);

    seqExpanded.set(parentNodeId, true);
    seqExpanded = new Map(seqExpanded);
  }

  function switchMode(newMode: 'cfg' | 'sequences' | 'session') {
    // Remove expanded nodes from graph store
    const toRemove = new Set<string>();
    for (const n of getNodes()) {
      if (n.data._type === 'block' || n.data._type === 'seq-next') {
        toRemove.add(n.id);
      }
    }
    if (toRemove.size > 0) removeNodesById(toRemove);
    resetAllDimmed();

    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    mode = newMode;
  }

  function highlightPath(funcName: string, path: any) {
    selectedPath = path;

    // Build set of highlighted block IDs
    const highlightedIds = new Set<string>(
      path.nodes.map((n: any) => `cfg:${funcName}:b${n.block_id}`)
    );

    // Build set of highlighted edge pairs (consecutive path nodes)
    const highlightedEdgePairs = new Set<string>();
    const blockIds = [...highlightedIds];
    for (let i = 0; i < blockIds.length - 1; i++) {
      highlightedEdgePairs.add(`${blockIds[i]}→${blockIds[i + 1]}`);
    }

    // Update nodes: dim all CFG blocks except highlighted ones
    setNodes(getNodes().map(n => {
      if (n.data._type === 'block' && n.data._parentFunc === funcName) {
        const dimmed = !highlightedIds.has(n.id);
        return { ...n, data: { ...n.data, _dimmed: dimmed } as GraphNodeData };
      }
      return n;
    }));

    // Update edges: dim all CFG edges except path edges
    setEdges(getEdges().map(e => {
      if (e.data?._parentFunc === funcName && e.data?._type === 'cfg-edge') {
        const key = `${e.source}→${e.target}`;
        const dimmed = !highlightedEdgePairs.has(key);
        return { ...e, style: edgeStyle(e.style, dimmed ? 0.1 : 1), data: { ...e.data, _dimmed: dimmed } };
      }
      return e;
    }));
  }

  // ── Cmd+K palette: publish context commands ──────────────────────────
  // Rebuilds whenever any input state changes (contract, scenarios,
  // active scenario, mode, canvas state, project map). The palette store
  // is global, so on route unmount we clear the list to avoid leaking
  // stale handlers back to the next page.
  $effect(() => {
    if (!contract) {
      setPaletteCommands([]);
      return;
    }
    const ctr = contract;
    const cmds: Command[] = [];

    // Modes — always present. Icon hints the layout.
    cmds.push(
      { id: 'mode:cfg', label: 'Mode: CFG', category: 'Mode', icon: '⊟', keywords: ['cfg', 'control flow'], run: () => switchMode('cfg') },
      { id: 'mode:sequences', label: 'Mode: Sequences', category: 'Mode', icon: '⇵', keywords: ['seq', 'calls'], run: () => switchMode('sequences') },
      { id: 'mode:session', label: 'Mode: Session', category: 'Mode', icon: '⎇', keywords: ['scenario', 'session'], run: () => switchMode('session') },
    );

    // Canvas / terminal actions.
    cmds.push(
      { id: 'canvas:center', label: 'Center canvas', category: 'Action', icon: '⊙', keywords: ['fit', 'zoom', 'reset view'], run: () => { flowApi?.fitView({ padding: 0.1 }); } },
      { id: 'canvas:clear', label: 'Clear canvas', category: 'Action', icon: '✕', keywords: ['reset', 'wipe'], run: () => {
        const names = Array.from(canvasFuncs);
        for (const n of names) removeFuncFromCanvas(n);
      } },
      { id: 'terminal:toggle', label: 'Toggle terminal', category: 'Action', icon: '>_', keywords: ['console', 'repl', 'pty'], run: () => toggleTerminal() },
    );

    // Session controls — only meaningful while there is an active scenario
    // with at least one step. We still expose them otherwise so users can
    // discover the shortcut; the handlers already guard empty scenarios.
    cmds.push(
      { id: 'session:back', label: 'Back — remove last step', category: 'Action', icon: '↶', keywords: ['undo', 'step'], run: () => handleSessionBack() },
      { id: 'session:clear', label: 'Clear scenario', category: 'Action', icon: '🗑', keywords: ['reset scenario'], run: () => handleSessionClear() },
    );

    // Scenario lifecycle. "Switch to X" and "Delete X" per existing
    // scenario; "New scenario" always available.
    cmds.push({
      id: 'scenario:new',
      label: 'New scenario',
      category: 'Scenario',
      icon: '+',
      keywords: ['create', 'scenario'],
      run: async () => {
        const name = promptScenarioName();
        if (!name) return;
        await dispatchScenarioAction({ New: { name } }, ctr.name, 'new');
      },
    });
    const activeScn = getActiveScenario();
    for (const [name] of getScenarios()) {
      if (name !== activeScn) {
        cmds.push({
          id: `scenario:switch:${name}`,
          label: `Switch to scenario: ${name}`,
          category: 'Scenario',
          icon: '⎇',
          run: () => dispatchScenarioAction({ Switch: { name } }, ctr.name, 'switch'),
        });
        cmds.push({
          id: `scenario:delete:${name}`,
          label: `Delete scenario: ${name}`,
          category: 'Scenario',
          icon: '✕',
          run: () => dispatchScenarioAction({ Delete: { name } }, ctr.name, 'delete'),
        });
      }
    }

    // Functions — own + inherited. Jump = add to canvas (Session mode
    // turns it into an add-step, which matches the sidebar click).
    // Solidity allows overloading by signature so names may repeat; we
    // include the index in the id to keep every row uniquely keyed even
    // when two rows end up with the same label.
    const allFuncs = [
      ...(ctr.functions ?? []).map((f) => ({ name: f.name, source: 'own' as const })),
      ...(ctr.inherited_functions ?? []).map((f) => ({ name: f.name, source: 'inherited' as const })),
    ];
    allFuncs.forEach((f, i) => {
      cmds.push({
        id: `func:${f.source}:${i}:${f.name}`,
        label: f.name,
        category: 'Function',
        icon: 'ƒ',
        detail: f.source === 'inherited' ? 'inherited' : undefined,
        keywords: ['jump', 'function', 'canvas'],
        run: () => handleSidebarAdd(f.name),
      });
    });

    // Cross-contract navigation. Skip the current one and dedupe by name
    // — ProjectMap may list the same interface twice (keyed each would
    // throw on duplicate ids).
    const seenContracts = new Set<string>([ctr.name]);
    for (const c of projectMap) {
      if (seenContracts.has(c.name)) continue;
      seenContracts.add(c.name);
      cmds.push({
        id: `contract:${c.name}`,
        label: c.name,
        category: 'Contract',
        icon: '◈',
        detail: c.kind,
        keywords: ['contract', 'navigate', 'open'],
        run: () => goto(`/contract/${encodeURIComponent(c.name)}`),
      });
    }

    setPaletteCommands(cmds);
  });

  // Clear published commands on unmount so the palette doesn't render
  // stale handlers if the user lands on a page that doesn't publish its
  // own list.
  onDestroy(() => {
    clearPaletteCommands();
  });
</script>

<div class="fixed inset-0 flex flex-col bg-dark">
  {#if error}
    <div class="p-6 text-danger">{error}</div>
  {:else if kind === 'solana' && solanaProgram}
    <div class="flex items-center gap-2.5 px-4 py-2 bg-hover border-b border-border-subtle z-10 shrink-0">
      <a class="text-text no-underline hover:text-accent-hover" href="/">ilold</a>
      <span class="text-text-dim">/</span>
      <span class="text-lg font-bold text-text">{solanaProgram.name}</span>
      <span class="text-xs text-text-muted">solana program</span>
      <span class="text-[10px] text-text-dim font-mono ml-2">{solanaProgram.program_id}</span>
      <div class="ml-auto flex gap-1">
        <button class="bg-hover border border-border-subtle text-accent-hover px-3 py-1 rounded-sm cursor-pointer text-xs hover:border-accent" onclick={togglePalette}>⌘K</button>
      </div>
    </div>
    <div class="flex-1 flex overflow-hidden h-full">
      <InstructionSidebar
        program={solanaProgram}
        canvasInstructions={solanaCanvasIxs}
        mode="program"
        onadd={(ix) => handleSolanaIxAdd(ix)}
        onremove={(ix) => handleSolanaIxRemove(ix)}
      />
      <GraphCanvasFlow
        bind:this={graphCanvas}
        onnodetap={(node, event) => handleNodeClick(node, event)}
        onbackgroundtap={handleBackgroundTap}
        oncontextmenu={handleContextMenu}
        onnodesdelete={handleNodesDelete}
        canDeleteNodes={true}
        onselectionchange={(nodes) => { selectionCount = nodes.length; }}
        onready={(api) => { flowApi = api; }}
      />
    </div>
  {:else}
    <TopBar
      contractName={contract?.name ?? '...'}
      {mode}
      {seqDirection}
      onmodechange={switchMode}
      onsearch={togglePalette}
      oncenter={() => flowApi?.fitView({ padding: 0.1 })}
      onseqdirection={(dir) => { seqDirection = dir; reorientAllSeqSubtrees(); }}
      onsessionback={handleSessionBack}
      onsessionclear={handleSessionClear}
    />
    <div class="flex-1 flex overflow-hidden h-full">
      {#if contract}
        <FunctionSidebar {contract} {canvasFuncs} {mode} onadd={handleSidebarAdd} onremove={removeFuncFromCanvas} />
      {/if}

      <GraphCanvasFlow
        bind:this={graphCanvas}
        onnodetap={(node, event) => handleNodeClick(node, event)}
        onbackgroundtap={handleBackgroundTap}
        oncontextmenu={handleContextMenu}
        onnodesdelete={handleNodesDelete}
        canDeleteNodes={mode !== 'session'}
        onselectionchange={(nodes) => { selectionCount = nodes.length; }}
        onready={(api) => { flowApi = api; }}
      />

      {#if contract}
        <SessionSidebar
          contract={contract.name}
          {selectedNode}
          {selectedPath}
          {funcPaths}
          {expandedFuncs}
          {seqExpanded}
          {mode}
          {seqAnalysis}
          contractDetail={{ name: contract.name, functions: contract.functions }}
          lookupBlock={(blockId) => {
            const node = findNode(blockId);
            if (!node || node.data._type !== 'block') return null;
            return { statements: (node.data as any).statements ?? [], node_type: (node.data as any).node_type };
          }}
          onpathselect={(funcName, path) => { selectedPath = path; highlightPath(funcName, path); }}
          onexpandcfg={(funcName, nodeId) => toggleFuncExpand(funcName, nodeId)}
        />
      {/if}
    </div>

    <StatusBar
      {mode}
      canvasCount={canvasFuncs.size}
      expandedCount={expandedFuncs.size}
      activeScenario={getActiveScenario()}
      {selectionCount}
    />

    <ContextMenu
      menu={contextMenu}
      {expandedFuncs}
      {seqExpanded}
      {mode}
      onexpandcfg={(func, nodeId) => { toggleFuncExpand(func, nodeId); contextMenu = null; }}
      onremovefunc={(func) => { removeFuncFromCanvas(func); contextMenu = null; selectedNode = null; }}
      onremovenode={(nodeId) => { removeSeqNode(nodeId); contextMenu = null; selectedNode = null; }}
      onforkscenario={handleForkScenario}
      onremovefromhere={handleRemoveFromHere}
      onviewsource={handleViewSource}
      onopenide={handleOpenInIde}
      onclose={() => contextMenu = null}
    />

    {#if sourcePanel && contract}
      <FunctionSourcePanel
        contract={contract.name}
        func={sourcePanel.func}
        onclose={() => sourcePanel = null}
      />
    {/if}

    <Legend {mode} />
  {/if}
</div>

