<script lang="ts">
  import { page } from '$app/state';
  import { onMount, tick } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, getSequenceAnalysis, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis } from '$lib/api/rest';
  import { toggleSearch, setSearchContext, getSearchNavigate, setSearchNavigate } from '$lib/stores/search.svelte';
  import Legend from '$lib/components/contract/Legend.svelte';
  import FunctionSidebar from '$lib/components/contract/FunctionSidebar.svelte';
  import FloatingToolbar from '$lib/components/contract/FloatingToolbar.svelte';
  import ContextMenu from '$lib/components/contract/ContextMenu.svelte';
  import BranchMenu from '$lib/components/contract/BranchMenu.svelte';
  import NodeDetailPanel from '$lib/components/contract/NodeDetailPanel.svelte';
  import GraphCanvasFlow from '$lib/components/contract/GraphCanvasFlow.svelte';
  import SessionSidebar from '$lib/components/session/SessionSidebar.svelte';
  import {
    getNodes, getEdges,
    setNodes, setEdges,
    addNode, addEdge,
    addNodes, addEdges,
    removeNodesById, findNode,
    findDescendants,
    type GraphNodeData,
  } from '$lib/stores/graph.svelte';
  import { runDagreLayout } from '$lib/utils/graph-helpers';
  import type { Node, Edge } from '@xyflow/svelte';

  let contract: ContractDetail | null = $state(null);
  let error: string | null = $state(null);
  let selectedNode: any = $state(null);
  let selectedPath: any = $state(null);
  let funcPaths: Record<string, any> = $state({});
  let expandedFuncs: Set<string> = $state(new Set());
  let mode: 'cfg' | 'sequences' = $state('cfg');
  let seqTree: any = $state(null);
  let seqAnalysis: SequenceAnalysis | null = $state(null);
  let seqExpanded: Map<string, boolean> = $state(new Map());
  let seqDirection: 'TB' | 'LR' = $state('TB');

  // Branch menu: Shift+click shows a menu to add a branch
  let branchMenu: { x: number; y: number; parentNodeId: string; parentFuncName: string } | null = $state(null);

  // Context menu: right-click on nodes
  let contextMenu: { x: number; y: number; nodeId: string; funcName: string; nodeType: string } | null = $state(null);

  let canvasFuncs: Set<string> = $state(new Set()); // functions currently on canvas

  let callgraphRaw: CytoscapeGraph | null = $state(null);
  let flowApi: { fitView: (opts?: any) => Promise<boolean> } | null = $state(null);
  let cfgCache: Record<string, CytoscapeGraph> = {};

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

  /** Remove non-branch descendants of a seq node, plus orphaned branches */
  function collapseNonBranchDescendants(nodeId: string) {
    const allDesc = findDescendants(nodeId);
    const toRemove = new Set<string>();
    for (const id of allDesc) {
      const n = findNode(id);
      if (n && n.data._type === 'seq-next' && !(n.data as any)._isBranch) {
        const subDesc = findDescendants(id);
        for (const sid of subDesc) toRemove.add(sid);
        toRemove.add(id);
      }
    }
    if (toRemove.size > 0) {
      // Also collect branches whose parent is being removed (they'd become orphans)
      for (const id of allDesc) {
        const n = findNode(id);
        if (n && (n.data as any)._isBranch && toRemove.has((n.data as any)._seqParent)) {
          toRemove.add(id);
        }
      }
      removeNodesById(toRemove);
    }
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

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    setSearchContext(contractName);
    try {
      contract = await getContract(contractName);
      const callgraphData = await getCallGraph(contractName);
      callgraphRaw = callgraphData;
      try { seqTree = await getSequences(contractName); } catch {}
      try { seqAnalysis = await getSequenceAnalysis(contractName); } catch {}
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  // Listen for search result navigation
  $effect(() => {
    const nav = getSearchNavigate();
    if (!nav || !contract) return;
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

    return () => { stale = true; };
  });

  function addFuncToCanvas(funcName: string) {
    if (!callgraphRaw || canvasFuncs.has(funcName)) return;
    const nodeData = callgraphRaw.nodes.find(n => n.data.label === funcName);
    if (!nodeData) return;

    // Look up enrichment data from ContractDetail
    const allFuncs = [...(contract?.functions ?? []), ...(contract?.inherited_functions ?? [])];
    const funcDetail = allFuncs.find((f: any) => f.name === funcName);

    const count = canvasFuncs.size;
    const x = 300 + (count % 3 - 1) * 200;
    const y = 200 + Math.floor(count / 3) * 100;

    addNode({
      id: nodeData.data.id,
      type: 'function',
      position: { x, y },
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
            type: 'default',
            data: { _type: 'call', kind: e.data.kind },
            ...(e.data.call_count > 1 ? {
              label: `\u00D7${e.data.call_count}`,
              labelStyle: 'fill: var(--color-text-muted); font-size: 10px',
              labelBgStyle: 'fill: var(--color-bg-surface)',
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
    branchMenu = null;
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
    branchMenu = null;
    resetAllDimmed();
  }

  function handleContextMenu(event: MouseEvent, node: Node<GraphNodeData>) {
    const data = node.data;
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      nodeId: node.id,
      funcName: data._type === 'function' ? data.label : ('_parentFunc' in data ? (data as any)._parentFunc : ('_funcName' in data ? (data as any)._funcName : (data as any).label)),
      nodeType: data._type,
    };
    branchMenu = null;
  }

  function handleNodeClick(node: Node<GraphNodeData>, event?: MouseEvent) {
    const d = node.data;
    if (d._type === 'function' && !d.is_external) {
      handleFunctionTap(d.label, node.id, event?.shiftKey ?? false, event);
    } else if (d._type === 'seq-next') {
      handleSeqNodeTap((d as any)._funcName || d.label, node.id, event?.shiftKey ?? false, !!(d as any)._isBranch, (d as any)._seqParent, event);
    }
    handleNodeTap(node);
  }

  async function handleFunctionTap(funcName: string, nodeId: string, shiftKey: boolean, event?: MouseEvent) {
    branchMenu = null;
    if (mode === 'cfg') {
      await toggleFuncExpand(funcName);
    } else if (mode === 'sequences') {
      if (shiftKey && event) {
        branchMenu = {
          x: event.clientX,
          y: event.clientY,
          parentNodeId: nodeId,
          parentFuncName: funcName,
        };
      } else {
        await toggleSeqExpand(funcName, nodeId);
      }
    }
  }

  async function handleSeqNodeTap(funcName: string, nodeId: string, shiftKey: boolean, isBranch: boolean, seqParent: string, event?: MouseEvent) {
    if (shiftKey && event) {
      branchMenu = {
        x: event.clientX,
        y: event.clientY,
        parentNodeId: nodeId,
        parentFuncName: funcName,
      };
      return;
    }

    // Remove auto-expanded siblings at same level first (collapse sibling trees)
    if (seqParent) {
      const siblings = getNodes().filter(
        n => n.data._type === 'seq-next'
          && (n.data as any)._seqParent === seqParent
          && n.id !== nodeId
      );
      for (const sib of siblings) {
        if (seqExpanded.has(sib.id)) {
          collapseNonBranchDescendants(sib.id);
          seqExpanded.delete(sib.id);
        }
      }
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
    const parentNode = findNode(parentId);
    const parentPos = parentNode?.position ?? { x: 300, y: 200 };

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

    // 2. Build edges
    const cfgEdges: Edge[] = cfg.edges.map((e, i) => ({
      id: `cfg-edge:${funcName}:${i}`,
      source: `cfg:${funcName}:${e.data.source}`,
      target: `cfg:${funcName}:${e.data.target}`,
      type: 'default',
      data: {
        _type: 'cfg-edge',
        _parentFunc: funcName,
        kind: e.data.kind,
      },
      label: e.data.kind.includes('ConditionalTrue') ? '✓'
           : e.data.kind.includes('ConditionalFalse') ? '✗'
           : undefined,
      animated: e.data.kind.includes('LoopBack'),
    }));

    // 3. Link edge: function node → CFG entry block
    const entryNode = cfg.nodes.find(n => n.data.node_type === 'Entry');
    if (entryNode) {
      cfgEdges.push({
        id: `cfg-link:${funcName}`,
        source: parentId,
        target: `cfg:${funcName}:${entryNode.data.id}`,
        type: 'default',
        data: { _type: 'cfg-edge', _parentFunc: funcName, kind: 'expand' },
        style: dashedEdgeStyle('var(--color-accent-dark)'),
      });
    }

    // 4. Run dagre on CFG subset to get positions
    const layoutNodes = runDagreLayout(cfgNodes, cfgEdges, {
      rankDir: 'TB', nodeSep: 30, rankSep: 45,
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

    // Add nodes at PARENT position (CSS transition will animate to final)
    addNodes(cfgNodes);
    addEdges(cfgEdges);

    // 6. After a tick, add expanding class + update positions
    await tick();

    // Add expanding class for animation
    for (const n of cfgNodes) {
      const el = document.querySelector(`[data-id="${n.id}"]`);
      el?.classList.add('expanding');
    }

    await tick();
    setNodes(getNodes().map(n => {
      const final = finalPositions.get(n.id);
      return final ? { ...n, position: final } : n;
    }));

    // Remove expanding class after animation completes
    setTimeout(() => {
      for (const n of cfgNodes) {
        const el = document.querySelector(`[data-id="${n.id}"]`);
        el?.classList.remove('expanding');
      }
    }, 350);

    // 7. Dim function nodes + call edges
    dimFunctionLayer(parentId);

    expandedFuncs.add(funcName);
    expandedFuncs = new Set(expandedFuncs);
  }

  function addBranch(parentNodeId: string, parentFuncName: string, branchFuncName: string) {
    if (!seqTree) return;

    const func = seqTree.functions.find((f: any) => f.name === branchFuncName);
    const transition = seqAnalysis?.transitions?.find(
      t => t.from === parentFuncName && t.to === branchFuncName
    ) ?? null;

    // Count existing children to offset position
    const existingChildren = getNodes().filter(
      n => n.data._type === 'seq-next' && (n.data as any)._seqParent === parentNodeId
    );
    const parentNode = findNode(parentNodeId);
    const parentPos = parentNode?.position ?? { x: 300, y: 200 };
    const isLR = seqDirection === 'LR';

    const offsetIdx = existingChildren.length;
    const nodeId = `seq-branch:${parentNodeId}→${branchFuncName}:${offsetIdx}`;

    const position = isLR
      ? { x: parentPos.x + 180, y: parentPos.y + offsetIdx * 50 }
      : { x: parentPos.x + offsetIdx * 160, y: parentPos.y + 60 };

    addNode({
      id: nodeId,
      type: 'sequence',
      position,
      data: {
        _type: 'seq-next',
        label: branchFuncName,
        _funcName: branchFuncName,
        _seqParent: parentNodeId,
        _isBranch: true,
        readOnly: func?.read_only ?? false,
        pathCount: func?.path_count,
        _transition: transition,
      },
    } as Node<GraphNodeData>);

    addEdge({
      id: `seq-edge:branch:${parentNodeId}→${branchFuncName}:${offsetIdx}`,
      source: parentNodeId,
      target: nodeId,
      type: 'default',
      data: { _type: 'seq-edge' },
      style: dashedEdgeStyle('var(--color-success)'),
    });

    branchMenu = null;
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    // ── COLLAPSE ──
    if (seqExpanded.has(parentNodeId)) {
      collapseNonBranchDescendants(parentNodeId);
      seqExpanded.delete(parentNodeId);
      seqExpanded = new Map(seqExpanded);

      // If no seq-next nodes remain, un-dim everything
      const anySeq = getNodes().some(n => n.data._type === 'seq-next');
      if (!anySeq) resetAllDimmed();
      return;
    }

    // ── EXPAND ──
    if (!seqTree || !seqTree.functions) return;
    const parentNode = findNode(parentNodeId);
    const parentPos = parentNode?.position ?? { x: 300, y: 200 };

    const seqFunctions: Array<{ name: string; visibility: string; read_only: boolean; path_count: number }> = seqTree.functions;

    const newNodes: Node<GraphNodeData>[] = [];
    const newEdges: Edge[] = [];

    for (const func of seqFunctions) {
      const targetName = func.name;
      const nodeId = `seq:${parentNodeId}→${targetName}`;

      // Look up transition from seqAnalysis
      const transition = seqAnalysis?.transitions?.find(
        t => t.from === funcName && t.to === targetName
      ) ?? null;

      newNodes.push({
        id: nodeId,
        type: 'sequence',
        position: { ...parentPos },
        data: {
          _type: 'seq-next',
          label: targetName,
          _funcName: targetName,
          _seqParent: parentNodeId,
          _isBranch: false,
          readOnly: func.read_only,
          pathCount: func.path_count,
          _transition: transition,
        },
      } as Node<GraphNodeData>);

      newEdges.push({
        id: `seq-edge:${parentNodeId}→${targetName}`,
        source: parentNodeId,
        target: nodeId,
        type: 'default',
        data: { _type: 'seq-edge' },
        style: transition?.shared_state?.length
          ? dashedEdgeStyle('var(--color-warning)')
          : undefined,
      });
    }

    // Run dagre layout on the seq subset
    const isLR = seqDirection === 'LR';
    const layoutNodes = runDagreLayout(newNodes, newEdges, {
      rankDir: seqDirection,
      nodeSep: 25,
      rankSep: 40,
    });

    // Offset below (TB) or beside (LR) parent
    let minX = Infinity, minY = Infinity, maxX = -Infinity;
    for (const n of layoutNodes) {
      if (n.position.x < minX) minX = n.position.x;
      if (n.position.x > maxX) maxX = n.position.x;
      if (n.position.y < minY) minY = n.position.y;
    }
    const centerX = (minX + maxX) / 2;
    const offsetX = isLR ? parentPos.x + 180 - minX : parentPos.x - centerX;
    const offsetY = isLR ? parentPos.y - minY : parentPos.y + 60 - minY;

    for (const n of layoutNodes) {
      n.position = { x: n.position.x + offsetX, y: n.position.y + offsetY };
    }

    // Merge layout positions back
    const posMap = new Map(layoutNodes.map(n => [n.id, n.position]));
    for (const n of newNodes) {
      const pos = posMap.get(n.id);
      if (pos) n.position = pos;
    }

    addNodes(newNodes);
    addEdges(newEdges);
    dimFunctionLayer();

    seqExpanded.set(parentNodeId, true);
    seqExpanded = new Map(seqExpanded);
  }

  function switchMode(newMode: 'cfg' | 'sequences') {
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


</script>

<div class="fixed inset-0 flex flex-col bg-dark">
  {#if error}
    <div class="p-6 text-danger">{error}</div>
  {:else}
    <FloatingToolbar
      contractName={contract?.name ?? '...'}
      {mode}
      {seqDirection}
      onmodechange={switchMode}
      onsearch={toggleSearch}
      oncenter={() => flowApi?.fitView({ padding: 0.1 })}
      onseqdirection={(dir) => { seqDirection = dir; }}
    />
    <div class="flex-1 flex overflow-hidden h-full">
      {#if contract}
        <FunctionSidebar {contract} {canvasFuncs} onadd={addFuncToCanvas} onremove={removeFuncFromCanvas} />
      {/if}

      <GraphCanvasFlow
        onnodetap={(node, event) => handleNodeClick(node, event)}
        onbackgroundtap={handleBackgroundTap}
        oncontextmenu={handleContextMenu}
        onready={(api) => { flowApi = api; }}
      />

      {#if contract}
        <SessionSidebar contract={contract.name} />
      {/if}
    </div>

    {#if selectedNode && contract}
      <NodeDetailPanel
        {selectedNode}
        {selectedPath}
        {funcPaths}
        {expandedFuncs}
        {seqExpanded}
        {mode}
        {seqAnalysis}
        contract={{ name: contract.name, functions: contract.functions }}
        lookupBlock={(blockId) => {
          const node = findNode(blockId);
          if (!node || node.data._type !== 'block') return null;
          return { statements: (node.data as any).statements ?? [], node_type: (node.data as any).node_type };
        }}
        onclose={() => { selectedNode = null; selectedPath = null; }}
        onpathselect={(funcName, path) => { selectedPath = path; highlightPath(funcName, path); }}
        onexpandcfg={(funcName, nodeId) => toggleFuncExpand(funcName, nodeId)}
      />
    {/if}

    {#if branchMenu && seqTree}
      <BranchMenu
        menu={branchMenu}
        functions={seqTree.functions}
        onselect={(parentNodeId, parentFuncName, func) => addBranch(parentNodeId, parentFuncName, func)}
        onclose={() => branchMenu = null}
      />
    {/if}

    <ContextMenu
      menu={contextMenu}
      {expandedFuncs}
      {seqExpanded}
      {mode}
      onexpandcfg={(func, nodeId) => { toggleFuncExpand(func, nodeId); contextMenu = null; }}
      onremovefunc={(func) => { removeFuncFromCanvas(func); contextMenu = null; selectedNode = null; }}
      onremovenode={(nodeId) => { removeSeqNode(nodeId); contextMenu = null; selectedNode = null; }}
      onaddbranch={(x, y, nodeId, func) => { branchMenu = { x, y, parentNodeId: nodeId, parentFuncName: func }; contextMenu = null; }}
      onclose={() => contextMenu = null}
    />

    <Legend {mode} />
  {/if}
</div>

