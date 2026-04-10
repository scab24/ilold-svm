<script lang="ts">
  import { page } from '$app/state';
  import { onMount, tick } from 'svelte';
  import { getContract, getCallGraph, getPaths, getSequences, getSequenceAnalysis, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis } from '$lib/api/rest';
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
    addNode, addEdge,
    removeNodesById, findNode,
    findDescendants,
    type GraphNodeData,
  } from '$lib/stores/graph.svelte';
  import type { Node } from '@xyflow/svelte';

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
      if (!canvasFuncs.has(nav.func)) {
        addFuncToCanvas(nav.func);
        await tick();
      }

      if (!funcPaths[nav.func]) {
        try {
          funcPaths[nav.func] = await getPaths(contract.name, nav.func);
          funcPaths = { ...funcPaths };
        } catch { return; }
      }

      if (stale) return;

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
        // TODO B3-2: fitView/setCenter to navigate to the node
      }

      if (!stale) setSearchNavigate(null);
    })();

    return () => { stale = true; };
  });

  function addFuncToCanvas(funcName: string) {
    if (!callgraphRaw || canvasFuncs.has(funcName)) return;
    const nodeData = callgraphRaw.nodes.find(n => n.data.label === funcName);
    if (!nodeData) return;

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

    canvasFuncs.delete(funcName);
    canvasFuncs = new Set(canvasFuncs);
    expandedFuncs.delete(funcName);
    seqExpanded.delete(nodeId);
  }

  // --- Event handlers ---

  async function handleNodeTap(node: Node<GraphNodeData>) {
    const data = node.data;

    if (!selectedNode || selectedNode.id !== node.id) {
      selectedPath = null;
      // Note: opacity reset deferred to B3-2 (highlightPath)
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
    // Note: opacity reset (cy.nodes('.block').style({ opacity: 1 })) deferred to B3-2
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
      handleSeqNodeTap((d as any)._funcName || d.label, node.id, event?.shiftKey ?? false, !!(d as any)._isBranch, (d as any)._seqParent);
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

  async function handleSeqNodeTap(funcName: string, nodeId: string, shiftKey: boolean, isBranch: boolean, seqParent: string) {
    // B3-3: full sequence expansion logic
    console.warn('handleSeqNodeTap: sequences deferred to B3-3');
  }

  async function toggleFuncExpand(funcName: string, anchorNodeId?: string) {
    // TODO B3-2: Port CFG expand/collapse to graph store + dagre layout
    console.warn(`[B3-2] toggleFuncExpand("${funcName}") — not yet ported`);
  }

  function addBranch(parentNodeId: string, parentFuncName: string, branchFuncName: string) {
    console.warn(`[B3-3] addBranch — not yet ported`);
    branchMenu = null;
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    console.warn(`[B3-3] toggleSeqExpand("${funcName}") — not yet ported`);
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

    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    mode = newMode;
  }

  function highlightPath(funcName: string, path: any) {
    // TODO B3-2: Port path highlighting to store-based opacity
    selectedPath = path;
    console.warn(`[B3-2] highlightPath("${funcName}") — not yet ported`);
  }


</script>

<div class="view">
  {#if error}
    <div class="error">{error}</div>
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
    <div class="workspace">
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

<style>
  .view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #121215; }

  .error { padding: 24px; color: #b05050; }

  .workspace { flex: 1; display: flex; overflow: hidden; height: 100%; }

</style>
