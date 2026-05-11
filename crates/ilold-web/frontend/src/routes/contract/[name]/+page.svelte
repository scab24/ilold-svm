<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick, untrack } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, getSequenceAnalysis, getFunctionSource, getInstructionSource, getProjectMap, getProgramView, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis, type MapContract, type MapProgram, type ProgramView, type IxView, type AccountView, type IxAccountView } from '$lib/api/rest';
  import {
    applyOverlayUpdate as applyRuntimeOverlayUpdate,
    clearOverlay as clearRuntimeOverlay,
    loadInitialOverlay as loadRuntimeOverlay,
    getCpiEdges,
  } from '$lib/stores/runtimeOverlay.svelte';
  import {
    loadUserLabels,
    clearUserLabels,
  } from '$lib/stores/userLabels.svelte';
  import { goto } from '$app/navigation';
  import { toggleTerminal } from '$lib/stores/terminal.svelte';
  import { openInIde } from '$lib/utils/ide-links';
  import { setSearchContext, getSearchNavigate, setSearchNavigate } from '$lib/stores/search.svelte';
  import { subscribe as subscribeWs } from '$lib/api/ws';
  import { togglePalette, setPaletteCommands, clearPaletteCommands } from '$lib/stores/palette.svelte';
  import type { Command } from '$lib/commands/registry';
  import { getHighlightedFunction, getScenarios, getActiveScenario, getForkOrigins } from '$lib/stores/session.svelte';
  import { composeScenarioTree, type ComposedNode } from '$lib/canvas/scenarios';
  import { promptScenarioName } from '$lib/scenarios/name';
  import { dispatchScenarioAction } from '$lib/scenarios/dispatch';
  import { postCommand, postSolanaCommand } from '$lib/api/session';
  import Legend from '$lib/components/contract/Legend.svelte';
  import FunctionSidebar from '$lib/components/contract/FunctionSidebar.svelte';
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
  let solanaProgram: ProgramView | null = $state(null);
  let kind: 'solidity' | 'solana' = $state('solidity');
  let solanaCanvasIxs: Set<string> = $state(new Set());
  let solanaExpandedIxs: Set<string> = $state(new Set());
  let solanaUsers: { name: string; pubkey: string; lamports: number }[] = $state([]);
  let solanaTraceCount = $state(0);
  let hideSystem = $state(false);
  type SolanaRuntimeInfo = {
    computeUnits: number;
    diffsCount: number;
    logsExcerpt: string[];
    error?: string | null;
  };
  let solanaRuntimeByStep: Map<string, SolanaRuntimeInfo> = $state(new Map());
  let error: string | null = $state(null);
  let selectedNode: any = $state(null);
  let selectedPath: any = $state(null);
  let funcPaths: Record<string, any> = $state({});
  let expandedFuncs: Set<string> = $state(new Set());
  let selectionCount: number = $state(0);
  let mode: 'cfg' | 'sequences' | 'session' = $state('sequences');
  let seqTree: any = $state(null);
  let seqAnalysis: SequenceAnalysis | null = $state(null);
  let seqExpanded: Map<string, boolean> = $state(new Map());
  let seqDirection: 'TB' | 'LR' = $state('TB');

  let contextMenu: {
    x: number;
    y: number;
    nodeId: string;
    funcName: string;
    nodeType: string;
    sessionStep?: { stepIndex: number };
  } | null = $state(null);

  let canvasFuncs: Set<string> = $state(new Set());

  let projectMap: MapContract[] = $state([]);

  let sourcePanel: { func: string } | null = $state(null);

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

  const SEQ_NODE_W = 220;
  const SEQ_NODE_H = 80;
  const SEQ_SIBLING_GAP = 30;
  const SEQ_LEVEL_GAP = 120;

  function relayoutSeqTree(rootId: string) {
    const root = findNode(rootId);
    if (!root) return;
    const rootPos = liveNodePosition(rootId) ?? root.position;

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

    const byLevel: string[][] = Array.from({ length: maxLevel + 1 }, () => []);
    for (const [id, lvl] of levels) byLevel[lvl].push(id);

    const isLR = seqDirection === 'LR';
    const posMap = new Map<string, { x: number; y: number }>();
    for (let lvl = 1; lvl <= maxLevel; lvl++) {
      const ids = byLevel[lvl];
      const count = ids.length;
      if (isLR) {
        const totalH = count * SEQ_NODE_H + (count - 1) * SEQ_SIBLING_GAP;
        const startY = rootPos.y + SEQ_NODE_H / 2 - totalH / 2;
        const x = rootPos.x + lvl * (SEQ_NODE_W + SEQ_LEVEL_GAP);
        ids.forEach((id, i) => {
          posMap.set(id, { x, y: startY + i * (SEQ_NODE_H + SEQ_SIBLING_GAP) });
        });
      } else {
        const totalW = count * SEQ_NODE_W + (count - 1) * SEQ_SIBLING_GAP;
        const startX = rootPos.x + SEQ_NODE_W / 2 - totalW / 2;
        const y = rootPos.y + lvl * (SEQ_NODE_H + SEQ_LEVEL_GAP);
        ids.forEach((id, i) => {
          posMap.set(id, { x: startX + i * (SEQ_NODE_W + SEQ_SIBLING_GAP), y });
        });
      }
    }

    setNodes(getNodes().map(n => {
      if (n.data._type === 'seq-next' && posMap.has(n.id)) {
        return { ...n, position: posMap.get(n.id)! };
      }
      return n;
    }));

    const sh = isLR ? 'r' : 'b';
    const th = isLR ? 'l' : 't';
    setEdges(getEdges().map(e => {
      if (subtreeEdgeIds.has(e.id)) {
        return { ...e, sourceHandle: sh, targetHandle: th };
      }
      return e;
    }));
  }

  function reorientAllSeqSubtrees() {
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

  function edgeStyle(base: string | undefined, opacity: number): string {
    const cleaned = (base ?? '').replace(/opacity:\s*[\d.]+;?/g, '').trim();
    const sep = cleaned && !cleaned.endsWith(';') ? '; ' : ' ';
    return `${cleaned}${cleaned ? sep : ''}opacity: ${opacity}`.trim();
  }

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
    return {
      color: 'var(--color-text-dim)',
      animated: false,
      sourceHandle: 'b',
      targetHandle: 't',
    };
  }

  $effect(() => {
    const scenarios = getScenarios();
    const forkOrigins = getForkOrigins();
    const active = getActiveScenario();
    if (kind === 'solana') {
      if (!solanaProgram) return;
      paintSolanaScenarioTree(scenarios, forkOrigins, active);
      return;
    }
    if (!contract || !callgraphRaw) return;

    const tree = composeScenarioTree(scenarios, forkOrigins);

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

  $effect(() => {
    const currentMode = mode;
    untrack(() => {
      setNodes(getNodes().map((n) => {
        const isSessionNode = (n.data as any)?._sessionStep === true;
        const shouldHide = currentMode === 'session' ? !isSessionNode : isSessionNode;
        return n.hidden === shouldHide ? n : { ...n, hidden: shouldHide };
      }));
      setEdges(getEdges().map((e) => {
        const isSessionEdge = (e.data as any)?._type === 'session-path';
        const shouldHide = currentMode === 'session' ? !isSessionEdge : isSessionEdge;
        return e.hidden === shouldHide ? e : { ...e, hidden: shouldHide };
      }));
    });
  });

  $effect(() => {
    if (!selectedNode) return;
    if (!findNode(selectedNode.id)) {
      selectedNode = null;
      selectedPath = null;
    }
  });

  $effect(() => {
    const funcName = sessionHighlight;
    if (!funcName) return;
    const node = getNodes().find(
      n => n.data._type === 'function' && n.data.label === funcName
    );
    if (node) selectedNode = node;
  });

  function snakeToPascal(s: string): string {
    return s
      .split('_')
      .filter((p) => p.length > 0)
      .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
      .join('');
  }

  function findAccountType(program: ProgramView, accountName: string): AccountView | undefined {
    const target = snakeToPascal(accountName);
    return program.accounts.find((a) => a.name === target);
  }

  function isAdminGated(program: ProgramView, ixName: string): boolean {
    return (program.admin_gated ?? []).includes(ixName);
  }

  function handleSolanaIxAdd(ixName: string) {
    if (!solanaProgram) return;
    if (solanaCanvasIxs.has(ixName)) {
      const existing = findNode(`ix:${ixName}`);
      if (existing && flowApi) flowApi.fitView({ nodes: [{ id: existing.id }], padding: 0.5, duration: 400 });
      return;
    }
    const ix = solanaProgram.instructions.find((i) => i.name === ixName);
    if (!ix) return;
    const idx = solanaCanvasIxs.size;
    addNode({
      id: `ix:${ixName}`,
      type: 'instruction',
      position: { x: idx * 220, y: 200 },
      data: {
        _type: 'instruction',
        label: ixName,
        programName: solanaProgram.name,
        programId: solanaProgram.program_id,
        args: ix.args ?? [],
        accountsCount: (ix.accounts ?? []).length,
        hasPdas: (ix.accounts ?? []).some((a) => a.pda != null),
        signers: (ix.accounts ?? []).filter((a) => a.signer).map((a) => a.name),
        adminGated: isAdminGated(solanaProgram, ixName),
        discriminator_hex: ix.discriminator_hex,
      },
    });
    solanaCanvasIxs = new Set([...solanaCanvasIxs, ixName]);
    paintCpiEdges();
    if (flowApi) flowApi.fitView({ nodes: [{ id: `ix:${ixName}` }], padding: 0.5, duration: 400 });
  }

  function handleSolanaIxRemove(ixName: string) {
    const next = new Set(solanaCanvasIxs);
    next.delete(ixName);
    solanaCanvasIxs = next;
    const ids = new Set<string>([`ix:${ixName}`]);
    for (const n of getNodes()) {
      const data: any = n.data;
      if (data?._type === 'account' && data.parentInstruction === ixName) ids.add(n.id);
    }
    removeNodesById(ids);
    const edgePrefix = `cpi:${ixName}->`;
    const filtered = getEdges().filter((e) => !e.id.startsWith(edgePrefix));
    if (filtered.length !== getEdges().length) setEdges(filtered);
    pruneOrphanExternals();
    solanaExpandedIxs = new Set([...solanaExpandedIxs].filter((n) => n !== ixName));
  }

  /** Remove external-program placeholder nodes that have no incoming cpi edge
   *  left. Keeps the canvas clean after an ix is removed or after the overlay
   *  drops samples for a target. */
  function pruneOrphanExternals() {
    const usedTargets = new Set<string>();
    for (const e of getEdges()) {
      if (e.id.startsWith('cpi:')) usedTargets.add(e.target);
    }
    const orphans = new Set<string>();
    for (const n of getNodes()) {
      if (n.id.startsWith('external:') && !usedTargets.has(n.id)) orphans.add(n.id);
    }
    if (orphans.size > 0) removeNodesById(orphans);
  }

  function paintSolanaScenarioTree(
    scenarios: Map<string, any[]>,
    forkOrigins: Map<string, any>,
    active: string,
  ) {
    untrack(() => {
      const toRemove = new Set<string>();
      for (const n of getNodes()) {
        if (n.id.startsWith('session:') || n.id.startsWith('trace:')) toRemove.add(n.id);
      }
      if (toRemove.size > 0) removeNodesById(toRemove);

      const tree = composeScenarioTree(scenarios, forkOrigins);
      if (tree.nodes.length === 0) {
        solanaTraceCount = 0;
        return;
      }

      const SESSION_BASE_X = 200;
      const SESSION_BASE_Y = 200;
      const SESSION_STEP_WIDTH = 240;
      const SESSION_LANE_HEIGHT = 130;

      const composedNodes = tree.nodes.map((cn) => {
        const runtimeKey = `${cn._scenario}:${cn.stepIndex}`;
        const runtime = solanaRuntimeByStep.get(runtimeKey);
        return {
          id: cn.id,
          type: 'trace',
          position: {
            x: SESSION_BASE_X + cn.stepIndex * SESSION_STEP_WIDTH,
            y: SESSION_BASE_Y + cn.lane * SESSION_LANE_HEIGHT,
          },
          data: {
            _type: 'trace',
            _sessionStep: true,
            _scenario: cn._scenario,
            _scenariosPassingThrough: cn._scenariosPassingThrough,
            _activeScenario: active,
            stepIndex: cn.stepIndex,
            label: `${cn.function} #${cn.stepIndex}`,
            instruction: cn.function,
            scenario: cn._scenario,
            computeUnits: runtime?.computeUnits ?? 0,
            diffsCount: runtime?.diffsCount ?? 0,
            logsExcerpt: runtime?.logsExcerpt ?? [],
            error: runtime?.error ?? null,
          } as any,
        } as Node<GraphNodeData>;
      });
      addNodes(composedNodes);

      const nodeScenarios = new Map<string, string[]>(
        tree.nodes.map((n) => [n.id, n._scenariosPassingThrough]),
      );

      const composedEdges = tree.edges.map((ce) => {
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

      solanaTraceCount = tree.nodes.length;

      const liveKeys = new Set(tree.nodes.map((n) => `${n._scenario}:${n.stepIndex}`));
      let mutated = false;
      const next = new Map(solanaRuntimeByStep);
      for (const k of next.keys()) {
        if (!liveKeys.has(k)) {
          next.delete(k);
          mutated = true;
        }
      }
      if (mutated) solanaRuntimeByStep = next;
    });
  }

  function isHiddenAccount(accName: string): boolean {
    if (!hideSystem || !solanaProgram?.system_accounts) return false;
    return solanaProgram.system_accounts.includes(accName);
  }

  function paintIxAccounts(ixName: string) {
    if (!solanaProgram) return;
    const ix = solanaProgram.instructions.find((i) => i.name === ixName);
    if (!ix) return;
    const parent = findNode(`ix:${ixName}`);
    const baseX = parent?.position?.x ?? 0;
    const baseY = (parent?.position?.y ?? 200) - 200;
    const accounts = (ix.accounts ?? []).filter((acc) => !isHiddenAccount(acc.name));
    const totalWidth = (accounts.length - 1) * 170;
    const newNodes: any[] = [];
    const newEdges: any[] = [];
    accounts.forEach((acc: IxAccountView, i: number) => {
      const id = `ix:${ixName}:acc:${acc.name}`;
      const matched = findAccountType(solanaProgram!, acc.name);
      newNodes.push({
        id,
        type: 'account',
        position: { x: baseX - totalWidth / 2 + i * 170, y: baseY },
        data: {
          _type: 'account',
          label: acc.name,
          programName: solanaProgram!.name,
          parentInstruction: ixName,
          fields: matched?.fields ?? [],
          discriminator_hex: matched?.discriminator_hex,
          account_type: matched?.name,
          signer: acc.signer,
          writable: acc.writable,
          pda: acc.pda != null,
          kind: acc.kind,
        },
      });
      const edgeColor = acc.writable ? 'var(--color-accent-hover)' : 'var(--color-text-muted)';
      newEdges.push({
        id: `e:${ixName}:${acc.name}`,
        source: `ix:${ixName}`,
        sourceHandle: 't',
        target: id,
        targetHandle: 'b',
        style: acc.writable ? `stroke: ${edgeColor};` : `stroke-dasharray: 5 3; stroke: ${edgeColor};`,
        markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: edgeColor },
      });
    });
    if (newNodes.length > 0) addNodes(newNodes);
    if (newEdges.length > 0) addEdges(newEdges);
  }

  /** Friendly labels for well-known Solana programs. Anything not in this map
   *  falls back to a base58-truncated id rendered by ExternalProgramNode. */
  const KNOWN_PROGRAMS: Record<string, string> = {
    '11111111111111111111111111111111': 'system_program',
    'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA': 'token_program',
    'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb': 'token_program_2022',
    'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL': 'associated_token_program',
    'SysvarRent111111111111111111111111111111111': 'sysvar:rent',
    'SysvarC1ock11111111111111111111111111111111': 'sysvar:clock',
  };

  function externalNodeId(programId: string): string {
    return `external:${programId}`;
  }
  function cpiEdgeId(fromIx: string, programId: string): string {
    return `cpi:${fromIx}->${programId}`;
  }

  /** Paint dashed `cpi` edges from on-canvas instruction nodes to placeholder
   *  external-program nodes. Reads from the runtime overlay store, so the
   *  same handler covers both the initial REST snapshot and incremental WS
   *  updates. Edges persist until the user clears the canvas or switches
   *  scenario; cleanup happens in clearGraph / scenario_switched. */
  function paintCpiEdges() {
    if (!solanaProgram) return;
    const edges = getCpiEdges();
    if (edges.length === 0) return;

    const existing = new Set(getEdges().map((e) => e.id));
    const liveNodes = new Set(getNodes().map((n) => n.id));
    const usedExternals = new Set<string>();
    const newNodes: any[] = [];
    const newEdges: any[] = [];

    for (const e of edges) {
      const fromId = `ix:${e.from_ix}`;
      if (!liveNodes.has(fromId)) continue;

      const extId = externalNodeId(e.to_program);
      const label = KNOWN_PROGRAMS[e.to_program] ?? e.to_program;

      if (!liveNodes.has(extId) && !usedExternals.has(extId)) {
        const parent = findNode(fromId);
        const baseX = (parent?.position?.x ?? 0) + 220;
        const baseY = (parent?.position?.y ?? 200) + 80;
        newNodes.push({
          id: extId,
          type: 'function',
          position: { x: baseX, y: baseY },
          data: {
            _type: 'function',
            label,
            is_external: true,
          },
        });
        usedExternals.add(extId);
      }

      const edgeId = cpiEdgeId(e.from_ix, e.to_program);
      if (existing.has(edgeId)) continue;
      newEdges.push({
        id: edgeId,
        source: fromId,
        sourceHandle: 'r',
        target: extId,
        targetHandle: 'l',
        label: `cpi (${e.samples}x)`,
        style: 'stroke-dasharray: 5 3; stroke: var(--color-warning);',
        markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: 'var(--color-warning)' },
        labelBgStyle: { fill: 'var(--color-surface)', fillOpacity: 0.85 },
        labelBgPadding: [3, 5] as [number, number],
        labelStyle: 'font-size: 9px; fill: var(--color-warning);',
        data: { _type: 'cpi-edge' },
      });
    }

    if (newNodes.length > 0) addNodes(newNodes);
    if (newEdges.length > 0) addEdges(newEdges);
  }

  /** Drop existing CPI placeholder nodes and edges. Used when switching
   *  scenarios or reloading the overlay snapshot — the new overlay state
   *  determines what gets repainted via paintCpiEdges. */
  function clearCpiEdges() {
    const ids = new Set<string>();
    for (const n of getNodes()) {
      if (n.id.startsWith('external:')) ids.add(n.id);
    }
    if (ids.size > 0) removeNodesById(ids);
    const remainingEdges = getEdges().filter((e) => !e.id.startsWith('cpi:'));
    if (remainingEdges.length !== getEdges().length) setEdges(remainingEdges);
  }

  function clearIxAccounts(ixName: string) {
    const ids = new Set<string>();
    for (const n of getNodes()) {
      const data: any = n.data;
      if (data?._type === 'account' && data.parentInstruction === ixName) ids.add(n.id);
    }
    if (ids.size > 0) removeNodesById(ids);
  }

  function handleSolanaIxExpand(ixName: string) {
    if (!solanaProgram) return;
    if (solanaExpandedIxs.has(ixName)) {
      clearIxAccounts(ixName);
      solanaExpandedIxs = new Set([...solanaExpandedIxs].filter((n) => n !== ixName));
      return;
    }
    paintIxAccounts(ixName);
    solanaExpandedIxs = new Set([...solanaExpandedIxs, ixName]);
  }

  function handleHideSystemToggle(next: boolean) {
    if (hideSystem === next) return;
    hideSystem = next;
    if (kind !== 'solana' || !solanaProgram) return;
    for (const ixName of solanaExpandedIxs) {
      clearIxAccounts(ixName);
      paintIxAccounts(ixName);
    }
  }

  function handleSolanaRun(name: string) {
    if (!solanaProgram) return;
    const ix = solanaProgram.instructions.find((i) => i.name === name);
    if (!ix) return;
    selectedNode = { ...findNode(`ix:${ix.name}`)?.data, id: `ix:${ix.name}` } as any;
    flowApi?.fitView({ nodes: [{ id: `ix:${ix.name}` }], padding: 0.5, duration: 400 });
  }

  async function handleSolanaSubmitFromInspector(
    ix: any,
    payload: { args: Record<string, any>; accounts: Record<string, string>; signers: string[] },
  ) {
    await handleSolanaSubmit(payload, ix);
  }

  onMount(() => {
    const unsub = subscribeWs('solana_users_changed', () => {
      if (solanaProgram) refreshSolanaUsers();
    });
    const unsubAdd = subscribeWs('session_add_node', (msg) => {
      const runtime = (msg as any).runtime;
      if (!runtime) return;
      const key = `${msg.scenario}:${msg.step_index}`;
      const logs: string[] = runtime.logs_excerpt ?? [];
      const inferredError = logs.find((l: string) =>
        l.includes('AnchorError') || l.includes('failed:') || l.includes('panicked')
      );
      const next = new Map(solanaRuntimeByStep);
      next.set(key, {
        computeUnits: runtime.compute_units ?? 0,
        diffsCount: runtime.diffs_count ?? 0,
        logsExcerpt: logs,
        error: runtime.error ?? inferredError ?? null,
      });
      solanaRuntimeByStep = next;
    });
    const unsubOverlay = subscribeWs('session_overlay_update', (msg) => {
      applyRuntimeOverlayUpdate(msg);
      paintCpiEdges();
    });
    const unsubScenarioSwitch = subscribeWs('scenario_switched', async (msg) => {
      if (solanaProgram) {
        clearCpiEdges();
        await loadRuntimeOverlay(solanaProgram.name, msg.to);
        await loadUserLabels(msg.to);
        paintCpiEdges();
      }
    });
    return () => { unsub(); unsubAdd(); unsubOverlay(); unsubScenarioSwitch(); };
  });

  async function refreshSolanaUsers() {
    if (!solanaProgram) return;
    try {
      const result = await postSolanaCommand('Users', solanaProgram.name);
      if (result?.UserList?.users) {
        solanaUsers = result.UserList.users;
      }
    } catch {}
  }

  async function handleSolanaSubmit(
    payload: {
      args: Record<string, any>;
      accounts: Record<string, string>;
      signers: string[];
    },
    targetIx?: any,
  ) {
    if (!solanaProgram) return;
    const ix = targetIx;
    if (!ix) return;
    const ixName = ix.name;
    const result = await postSolanaCommand(
      {
        Call: {
          ix: ixName,
          args: payload.args,
          accounts: payload.accounts,
          signers: payload.signers,
        },
      },
      solanaProgram.name,
    );
    if (result?.Error) {
      throw new Error(result.Error.message ?? 'Call failed');
    }
    if (result?.StepAdded) {
      const sa = result.StepAdded;
      const scenario = getActiveScenario() ?? 'main';
      const runtimeKey = `${scenario}:${sa.step_index}`;
      const next = new Map(solanaRuntimeByStep);
      const explicitError: string | null = (sa.error as string | null | undefined) ?? null;
      const logs: string[] = sa.logs_excerpt ?? [];
      const inferredError = logs.find((l) =>
        l.includes('AnchorError') || l.includes('failed:') || l.includes('panicked')
      );
      next.set(runtimeKey, {
        computeUnits: sa.compute_units ?? 0,
        diffsCount: sa.account_diffs_count ?? 0,
        logsExcerpt: logs,
        error: explicitError ?? inferredError ?? null,
      });
      solanaRuntimeByStep = next;
    }
    await refreshSolanaUsers();
  }

  let mountCancelled = $state(false);
  onDestroy(() => {
    mountCancelled = true;
    clearRuntimeOverlay();
    clearUserLabels();
  });

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    const expected = contractName;
    const stillFresh = () => !mountCancelled && page.params.name === expected;
    clearGraph();
    canvasFuncs = new Set();
    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    clearRuntimeOverlay();
    clearUserLabels();
    setSearchContext(contractName);
    try {
      const pm = await getProjectMap();
      if (!stillFresh()) return;
      kind = pm.kind === 'solana' ? 'solana' : 'solidity';
      if (kind === 'solana') {
        try {
          const prog = await getProgramView(contractName);
          if (!stillFresh()) return;
          solanaProgram = prog;
        } catch {
          if (stillFresh()) error = `Program "${contractName}" not found`;
          return;
        }
        projectMap = [];
        await refreshSolanaUsers();
        const scenario = getActiveScenario();
        await loadRuntimeOverlay(contractName, scenario);
        if (scenario) await loadUserLabels(scenario);
        paintCpiEdges();
        return;
      }
      projectMap = pm.contracts ?? [];
      const ctr = await getContract(contractName);
      if (!stillFresh()) return;
      contract = ctr;
      const callgraphData = await getCallGraph(contractName);
      if (!stillFresh()) return;
      callgraphRaw = callgraphData;
      try {
        const tree = await getSequences(contractName);
        if (stillFresh()) seqTree = tree;
      } catch (e) {
        if (stillFresh() && kind !== 'solana') console.warn('getSequences failed:', e);
      }
      try {
        const analysis = await getSequenceAnalysis(contractName);
        if (stillFresh()) seqAnalysis = analysis;
      } catch (e) {
        if (stillFresh() && kind !== 'solana') console.warn('getSequenceAnalysis failed:', e);
      }
    } catch (e) {
      if (stillFresh()) error = `Contract "${contractName}" not found`;
    }
  });

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

  function notifyFailure(label: string, e: unknown) {
    const reason = e instanceof Error ? e.message : String(e);
    console.warn(`${label} failed:`, e);
    alert(`${label} failed:\n\n${reason}`);
  }

  async function handleSidebarAdd(funcName: string) {
    if (kind === 'solana') {
      handleSolanaIxAdd(funcName);
      return;
    }
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

    for (const n of getNodes()) {
      if ('_parentFunc' in n.data && n.data._parentFunc === funcName) {
        toRemove.add(n.id);
      }
    }

    const seqDesc = findDescendants(nodeId);
    for (const id of seqDesc) toRemove.add(id);

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


  async function handleNodeTap(node: Node<GraphNodeData>) {
    const data = node.data;

    if (!selectedNode || selectedNode.id !== node.id) {
      selectedPath = null;
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

  async function handleSessionBack() {
    if (kind === 'solana' && solanaProgram) {
      try {
        await postSolanaCommand('Back', solanaProgram.name);
      } catch (e) {
        notifyFailure('session back', e);
      }
      return;
    }
    try {
      await postCommand('Back', contract?.name);
    } catch (e) {
      notifyFailure('session back', e);
    }
  }

  async function handleSessionClear() {
    const active = getActiveScenario();
    const steps = getScenarios().get(active) ?? [];
    if (steps.length === 0) return;
    if (!confirm(`Clear all ${steps.length} step(s) from scenario "${active}"?`)) return;
    if (kind === 'solana' && solanaProgram) {
      try {
        await postSolanaCommand('Clear', solanaProgram.name);
      } catch (e) {
        notifyFailure('session clear', e);
      }
      return;
    }
    try {
      await postCommand('Clear', contract?.name);
    } catch (e) {
      notifyFailure('session clear', e);
    }
  }

  function handleViewSource(funcName: string) {
    contextMenu = null;
    sourcePanel = { func: funcName };
  }

  async function handleOpenInIde(funcName: string) {
    contextMenu = null;
    const projectName = kind === 'solana' ? solanaProgram?.name : contract?.name;
    if (!projectName) return;
    try {
      const res = kind === 'solana'
        ? await getInstructionSource(projectName, funcName)
        : await getFunctionSource(projectName, funcName);
      openInIde(res.file_path, res.span.start_line, res.span.start_col);
    } catch (e) {
      notifyFailure('open in IDE', e);
    }
  }

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
    let sessionStep: { stepIndex: number } | undefined;
    if ((data._type === 'function' || data._type === 'trace') && data._sessionStep === true) {
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
    handleNodeTap(node);
    if (mode === 'session') return;
    const d = node.data;
    if (d._type === 'instruction' && mode === 'cfg') {
      handleSolanaIxExpand(d.label as string);
      return;
    }
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

    if (!cfgCache[funcName]) {
      cfgCache[funcName] = await getCfg(contract.name, funcName);
    }
    const cfg = cfgCache[funcName];
    const parentPos = liveNodePosition(parentId) ?? { x: 300, y: 200 };

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

    const layoutNodes = runDagreLayout(cfgNodes, cfgEdges, {
      rankDir: 'TB', nodeSep: 40, rankSep: 60, nodeWidth: 180,
    });

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

    for (const n of cfgNodes) {
      const final = finalPositions.get(n.id);
      if (final) n.position = final;
    }
    addNodes(cfgNodes);
    addEdges(cfgEdges);

    dimFunctionLayer(parentId);

    expandedFuncs.add(funcName);
    expandedFuncs = new Set(expandedFuncs);
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    if (seqExpanded.has(parentNodeId)) {
      collapseAllDescendants(parentNodeId);
      seqExpanded.delete(parentNodeId);
      seqExpanded = new Map(seqExpanded);

      const anySeq = getNodes().some(n => n.data._type === 'seq-next');
      if (!anySeq) resetAllDimmed();
      return;
    }

    if (!seqTree || !seqTree.functions) return;

    const rootFunc = findSeqRootFunction(parentNodeId);
    if (!rootFunc) return;

    const seqFunctions: Array<{ name: string; visibility: string; read_only: boolean; path_count: number }> = seqTree.functions;

    const targets = seqFunctions;

    const allFuncs = [...(contract?.functions ?? []), ...(contract?.inherited_functions ?? [])];

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

    addNodes(newNodes);
    addEdges(newEdges);
    relayoutSeqTree(rootFunc.id);

    dimFunctionLayer(rootFunc.id);

    seqExpanded.set(parentNodeId, true);
    seqExpanded = new Map(seqExpanded);
  }

  function switchMode(newMode: 'cfg' | 'sequences' | 'session') {
    if (kind === 'solana') {
      const toRemove = new Set<string>();
      for (const n of getNodes()) {
        const isSessionNode = (n.data as any)?._sessionStep === true;
        if (!isSessionNode) toRemove.add(n.id);
      }
      if (toRemove.size > 0) removeNodesById(toRemove);
      solanaCanvasIxs = new Set();
      solanaExpandedIxs = new Set();
      selectedNode = null;
      mode = newMode;
      return;
    }
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

    const highlightedIds = new Set<string>(
      path.nodes.map((n: any) => `cfg:${funcName}:b${n.block_id}`)
    );

    const highlightedEdgePairs = new Set<string>();
    const blockIds = [...highlightedIds];
    for (let i = 0; i < blockIds.length - 1; i++) {
      highlightedEdgePairs.add(`${blockIds[i]}→${blockIds[i + 1]}`);
    }

    setNodes(getNodes().map(n => {
      if (n.data._type === 'block' && n.data._parentFunc === funcName) {
        const dimmed = !highlightedIds.has(n.id);
        return { ...n, data: { ...n.data, _dimmed: dimmed } as GraphNodeData };
      }
      return n;
    }));

    setEdges(getEdges().map(e => {
      if (e.data?._parentFunc === funcName && e.data?._type === 'cfg-edge') {
        const key = `${e.source}→${e.target}`;
        const dimmed = !highlightedEdgePairs.has(key);
        return { ...e, style: edgeStyle(e.style, dimmed ? 0.1 : 1), data: { ...e.data, _dimmed: dimmed } };
      }
      return e;
    }));
  }

  $effect(() => {
    if (kind === 'solana' && solanaProgram) {
      const prog = solanaProgram;
      const cmds: Command[] = [];
      cmds.push(
        { id: 'mode:cfg', label: 'Mode: Program', category: 'Mode', icon: '⊟', keywords: ['cfg', 'program', 'instructions'], run: () => switchMode('cfg') },
        { id: 'mode:sequences', label: 'Mode: Sequences', category: 'Mode', icon: '⇵', keywords: ['seq', 'flow'], run: () => switchMode('sequences') },
        { id: 'mode:session', label: 'Mode: Session', category: 'Mode', icon: '⎇', keywords: ['scenario', 'session', 'trace'], run: () => switchMode('session') },
      );
      cmds.push({
        id: 'canvas:center',
        label: 'Center canvas',
        category: 'Action',
        icon: '⊙',
        keywords: ['fit', 'zoom', 'reset view'],
        run: () => { flowApi?.fitView({ padding: 0.1 }); },
      });
      for (const ix of prog.instructions ?? []) {
        cmds.push({
          id: `solana-ix:${ix.name}`,
          label: ix.name,
          category: 'Function',
          icon: 'ƒ',
          detail: `${(ix.args ?? []).length} args · ${(ix.accounts ?? []).length} accounts`,
          keywords: ['instruction', 'jump', 'canvas'],
          run: () => handleSolanaIxAdd(ix.name),
        });
        cmds.push({
          id: `solana-run:${ix.name}`,
          label: `Execute ${ix.name}`,
          category: 'Action',
          icon: '▶',
          keywords: ['call', 'execute', 'instruction', 'run'],
          run: () => handleSolanaRun(ix.name),
        });
      }
      for (const a of prog.accounts ?? []) {
        cmds.push({
          id: `solana-acc:${a.name}`,
          label: a.name,
          category: 'Contract',
          icon: '◇',
          detail: 'account type',
          keywords: ['account', 'type'],
          run: () => {
            const node = findNode(`account:${a.name}`);
            if (node && flowApi) flowApi.fitView({ nodes: [{ id: node.id }], padding: 0.5, duration: 400 });
          },
        });
      }
      setPaletteCommands(cmds);
      return;
    }

    if (!contract) {
      setPaletteCommands([]);
      return;
    }
    const ctr = contract;
    const cmds: Command[] = [];

    cmds.push(
      { id: 'mode:cfg', label: 'Mode: CFG', category: 'Mode', icon: '⊟', keywords: ['cfg', 'control flow'], run: () => switchMode('cfg') },
      { id: 'mode:sequences', label: 'Mode: Sequences', category: 'Mode', icon: '⇵', keywords: ['seq', 'calls'], run: () => switchMode('sequences') },
      { id: 'mode:session', label: 'Mode: Session', category: 'Mode', icon: '⎇', keywords: ['scenario', 'session'], run: () => switchMode('session') },
    );

    cmds.push(
      { id: 'canvas:center', label: 'Center canvas', category: 'Action', icon: '⊙', keywords: ['fit', 'zoom', 'reset view'], run: () => { flowApi?.fitView({ padding: 0.1 }); } },
      { id: 'canvas:clear', label: 'Clear canvas', category: 'Action', icon: '✕', keywords: ['reset', 'wipe'], run: () => {
        const names = Array.from(canvasFuncs);
        for (const n of names) removeFuncFromCanvas(n);
      } },
      { id: 'terminal:toggle', label: 'Toggle terminal', category: 'Action', icon: '>_', keywords: ['console', 'repl', 'pty'], run: () => toggleTerminal() },
    );

    cmds.push(
      { id: 'session:back', label: 'Back — remove last step', category: 'Action', icon: '↶', keywords: ['undo', 'step'], run: () => handleSessionBack() },
      { id: 'session:clear', label: 'Clear scenario', category: 'Action', icon: '🗑', keywords: ['reset scenario'], run: () => handleSessionClear() },
    );

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

  onDestroy(() => {
    clearPaletteCommands();
  });
</script>

<div class="fixed inset-0 flex flex-col bg-dark">
  {#if error}
    <div class="p-6 text-danger">{error}</div>
  {:else if contract || (kind === 'solana' && solanaProgram)}
    <TopBar
      contractName={kind === 'solana' && solanaProgram ? solanaProgram.name : (contract?.name ?? '...')}
      {mode}
      {seqDirection}
      {kind}
      {hideSystem}
      onmodechange={switchMode}
      onsearch={togglePalette}
      oncenter={() => flowApi?.fitView({ padding: 0.1 })}
      onseqdirection={(dir) => { seqDirection = dir; reorientAllSeqSubtrees(); }}
      onsessionback={handleSessionBack}
      onsessionclear={handleSessionClear}
      onhidesystem={handleHideSystemToggle}
    />
    <div class="flex-1 flex overflow-hidden h-full">
      <FunctionSidebar
        contract={kind === 'solana' ? null : contract}
        program={kind === 'solana' ? solanaProgram : null}
        {kind}
        canvasFuncs={kind === 'solana' ? solanaCanvasIxs : canvasFuncs}
        {mode}
        onadd={handleSidebarAdd}
        onremove={kind === 'solana' ? handleSolanaIxRemove : removeFuncFromCanvas}
      />

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

      <SessionSidebar
        contract={kind === 'solana' && solanaProgram ? solanaProgram.name : (contract?.name ?? '')}
        {kind}
        program={kind === 'solana' ? solanaProgram : null}
        {selectedNode}
        {selectedPath}
        {funcPaths}
        {expandedFuncs}
        {seqExpanded}
        {mode}
        {seqAnalysis}
        contractDetail={kind === 'solana' && solanaProgram
          ? { name: solanaProgram.name, functions: [] }
          : (contract ? { name: contract.name, functions: contract.functions } : null)}
        lookupBlock={(blockId) => {
          const node = findNode(blockId);
          if (!node || node.data._type !== 'block') return null;
          return { statements: (node.data as any).statements ?? [], node_type: (node.data as any).node_type };
        }}
        onpathselect={(funcName, path) => { selectedPath = path; highlightPath(funcName, path); }}
        onexpandcfg={(funcName, nodeId) => toggleFuncExpand(funcName, nodeId)}
        solanaUsers={solanaUsers}
        onsolanarun={handleSolanaRun}
        onsolanasubmit={handleSolanaSubmitFromInspector}
        onnewuser={async (name, lamports) => {
          if (!solanaProgram) return;
          const result = await postSolanaCommand({ UsersNew: { name, lamports } }, solanaProgram.name);
          if (result?.Error) throw new Error(result.Error.message ?? 'create user failed');
          await refreshSolanaUsers();
        }}
        onairdrop={async (name, lamports) => {
          if (!solanaProgram) return;
          await postSolanaCommand({ Airdrop: { user: name, lamports } }, solanaProgram.name);
          await refreshSolanaUsers();
        }}
      />
    </div>

    <StatusBar
      {mode}
      canvasCount={kind === 'solana' ? solanaCanvasIxs.size : canvasFuncs.size}
      expandedCount={kind === 'solana' ? solanaTraceCount : expandedFuncs.size}
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
      onremovenode={(nodeId) => {
        if (nodeId.startsWith('ix:')) {
          handleSolanaIxRemove(nodeId.slice(3));
        } else {
          removeSeqNode(nodeId);
        }
        contextMenu = null;
        selectedNode = null;
      }}
      onforkscenario={handleForkScenario}
      onremovefromhere={handleRemoveFromHere}
      onviewsource={handleViewSource}
      onopenide={handleOpenInIde}
      onsolanarun={(name) => { handleSolanaRun(name); contextMenu = null; }}
      onclose={() => contextMenu = null}
    />

    {#if sourcePanel && (contract || (kind === 'solana' && solanaProgram))}
      <FunctionSourcePanel
        contract={kind === 'solana' ? solanaProgram!.name : contract!.name}
        func={sourcePanel.func}
        kind={kind}
        onclose={() => sourcePanel = null}
      />
    {/if}

    <Legend {mode} {kind} />
  {/if}
</div>

