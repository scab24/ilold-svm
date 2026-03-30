<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, getSequenceAnalysis, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis } from '$lib/api/rest';
  import { toggleSearch, setSearchContext, searchNavigate } from '$lib/stores/search';
  import DraggablePanel from '$lib/DraggablePanel.svelte';
  import Collapsible from '$lib/Collapsible.svelte';

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
  let seqBreadcrumb: string[] = $state([]);
  let seqDirection: 'TB' | 'LR' = $state('TB');

  // Branch menu: Shift+click shows a menu to add a branch
  let branchMenu: { x: number; y: number; parentNodeId: string; parentFuncName: string } | null = $state(null);

  // Context menu: right-click on nodes
  let contextMenu: { x: number; y: number; nodeId: string; funcName: string; nodeType: string } | null = $state(null);

  // Draggable toolbar
  let toolbarX = $state(0);
  let toolbarY = $state(10);
  let toolbarDragging = false;
  let toolbarOffX = 0;
  let toolbarOffY = 0;
  function onToolbarDown(e: MouseEvent) {
    if ((e.target as HTMLElement).tagName === 'BUTTON' || (e.target as HTMLElement).tagName === 'A') return;
    toolbarDragging = true;
    toolbarOffX = e.clientX - toolbarX;
    toolbarOffY = e.clientY - toolbarY;
    window.addEventListener('mousemove', onToolbarMove);
    window.addEventListener('mouseup', onToolbarUp);
  }
  function onToolbarMove(e: MouseEvent) {
    if (!toolbarDragging) return;
    toolbarX = e.clientX - toolbarOffX;
    toolbarY = Math.max(0, e.clientY - toolbarOffY);
  }
  function onToolbarUp() {
    toolbarDragging = false;
    window.removeEventListener('mousemove', onToolbarMove);
    window.removeEventListener('mouseup', onToolbarUp);
  }

  // Sidebar: functions panel
  let sidebarOpen: boolean = $state(true);
  let canvasFuncs: Set<string> = $state(new Set()); // functions currently on canvas

  let cyContainer: HTMLDivElement;
  let canvasWrap: HTMLDivElement;
  let cyInstance: any = null;
  let dagreRegistered = false;
  let cfgCache: Record<string, CytoscapeGraph> = {};

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    setSearchContext(contractName);
    try {
      contract = await getContract(contractName);
      const callgraph = await getCallGraph(contractName);
      try { seqTree = await getSequences(contractName); } catch {}
      try { seqAnalysis = await getSequenceAnalysis(contractName); } catch {}
      await tick();
      await new Promise(r => requestAnimationFrame(r));
      if (cyContainer && callgraph) renderGraph(callgraph);
      toolbarX = Math.floor(window.innerWidth / 2 - 150);
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  // Listen for search result navigation
  const unsubSearch = searchNavigate.subscribe(async (nav) => {
    if (!nav || !cyInstance || !contract) return;
    if (nav.contract !== contract.name) return;

    // Add function to canvas if not already there
    if (!canvasFuncs.has(nav.func)) {
      addFuncToCanvas(nav.func);
      await tick();
    }

    // Load paths if needed
    if (!funcPaths[nav.func]) {
      try {
        funcPaths[nav.func] = await getPaths(contract.name, nav.func);
        funcPaths = { ...funcPaths };
      } catch { return; }
    }

    // Expand CFG if not already expanded
    if (!expandedFuncs.has(nav.func)) {
      await toggleFuncExpand(nav.func);
    }

    // Select the function node and the path
    const funcNode = cyInstance.nodes().filter((n: any) => n.data('label') === nav.func && n.data('_type') === 'function');
    if (funcNode.length) {
      selectedNode = funcNode.data();
      const path = funcPaths[nav.func]?.paths?.find((p: any) => p.id === nav.pathId);
      if (path) {
        highlightPath(nav.func, path);
      }
      cyInstance.animate({ center: { eles: funcNode }, zoom: cyInstance.zoom() }, { duration: 300 });
    }

    searchNavigate.set(null);
  });

  onDestroy(() => {
    unsubSearch();
    if (cyInstance) { cyInstance.destroy(); cyInstance = null; }
  });

  let callgraphData: CytoscapeGraph | null = null;

  function addFuncToCanvas(funcName: string) {
    if (!cyInstance || !callgraphData || canvasFuncs.has(funcName)) return;
    const nodeData = callgraphData.nodes.find(n => n.data.label === funcName);
    if (!nodeData) return;

    const center = cyInstance.extent();
    const x = (center.x1 + center.x2) / 2 + (canvasFuncs.size % 3 - 1) * 180;
    const y = (center.y1 + center.y2) / 2 + Math.floor(canvasFuncs.size / 3) * 70;

    cyInstance.add({
      group: 'nodes',
      data: { ...nodeData.data, _type: 'function' },
      classes: nodeData.data.is_external ? 'external' : 'internal',
      position: { x, y },
    });

    canvasFuncs.add(funcName);
    canvasFuncs = new Set(canvasFuncs);
  }

  function removeSeqNode(nodeId: string) {
    if (!cyInstance) return;
    const node = cyInstance.getElementById(nodeId);
    if (!node.length) return;
    // Remove all descendants recursively
    const toRemove = cyInstance.collection();
    const toRemoveEdges = cyInstance.collection();
    function collect(nid: string) {
      const ch = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === nid);
      ch.forEach((c: any) => { toRemove.merge(c); collect(c.id()); });
      toRemoveEdges.merge(cyInstance.edges().filter((e: any) => e.data('_seqParent') === nid));
    }
    collect(nodeId);
    // Also remove edges pointing to this node
    toRemoveEdges.merge(node.connectedEdges());
    toRemove.merge(node);
    cyInstance.remove(toRemoveEdges);
    cyInstance.remove(toRemove);
    seqExpanded.delete(nodeId);
    for (const k of seqExpanded.keys()) {
      if (cyInstance.getElementById(k).length === 0) seqExpanded.delete(k);
    }
  }

  function removeFuncFromCanvas(funcName: string) {
    if (!cyInstance || !canvasFuncs.has(funcName)) return;
    const node = cyInstance.nodes().filter((n: any) => n.data('label') === funcName && n.data('_type') === 'function');
    if (!node.length) return;
    const nodeId = node.id();
    // Remove all descendants
    const desc = cyInstance.nodes().filter((n: any) => {
      const sp = n.data('_seqParent');
      return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
    });
    const descEdges = cyInstance.edges().filter((e: any) => {
      const sp = e.data('_seqParent');
      return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
    });
    cyInstance.remove(descEdges);
    cyInstance.remove(desc);
    // Remove CFG children
    cyInstance.remove(cyInstance.nodes(`[_parentFunc = "${funcName}"]`));
    cyInstance.remove(cyInstance.edges(`[_parentFunc = "${funcName}"]`));
    // Remove the node itself and its call edges
    cyInstance.remove(node.connectedEdges());
    cyInstance.remove(node);
    canvasFuncs.delete(funcName);
    canvasFuncs = new Set(canvasFuncs);
    expandedFuncs.delete(funcName);
    seqExpanded.delete(nodeId);
  }

  async function renderGraph(graph: CytoscapeGraph) {
    callgraphData = graph;
    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    // Start with empty canvas — functions are added from sidebar
    cyInstance = cytoscape({
      container: cyContainer,
      elements: [],
      style: getStyles() as any,
      layout: { name: 'preset' },
      minZoom: 0.1, maxZoom: 5, wheelSensitivity: 0.3,
    });
    canvasFuncs = new Set();

    // Single click on function nodes
    cyInstance.on('tap', 'node.internal', async (evt: any) => {
      const data = evt.target.data();
      if (data._type !== 'function') return;
      branchMenu = null;
      if (mode === 'cfg') {
        await toggleFuncExpand(data.label);
      } else if (mode === 'sequences') {
        if (evt.originalEvent?.shiftKey) {
          const rect = cyContainer.getBoundingClientRect();
          const pos = evt.renderedPosition || evt.position;
          branchMenu = { x: pos.x + rect.left, y: pos.y + rect.top, parentNodeId: data.id, parentFuncName: data.label };
        } else {
          await toggleSeqExpand(data.label, data.id);
        }
      }
    });

    // Single click on seq-next nodes
    cyInstance.on('tap', 'node.seq-next', async (evt: any) => {
      const data = evt.target.data();
      const funcName = data._funcName || data.label;
      const nodeId = data.id;
      branchMenu = null;

      if (evt.originalEvent?.shiftKey) {
        const rect = cyContainer.getBoundingClientRect();
        const pos = evt.renderedPosition || evt.position;
        branchMenu = { x: pos.x + rect.left, y: pos.y + rect.top, parentNodeId: nodeId, parentFuncName: funcName };
        return;
      }

      if (seqExpanded.has(nodeId)) {
        await toggleSeqExpand(funcName, nodeId);
        return;
      }

      // If this is a branch node, just expand it — don't touch siblings
      if (data._isBranch) {
        await toggleSeqExpand(funcName, nodeId);
        return;
      }

      // Auto-expanded node: remove other auto-expanded siblings (keep ALL branches everywhere)
      const parentKey = data._seqParent;
      const me = cyInstance.getElementById(nodeId);
      const siblings = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === parentKey && n.id() !== nodeId && !n.data('_isBranch'));
      const toRemove = cyInstance.collection();
      const toRemoveEdges = cyInstance.collection();
      // Recursively collect descendants but NEVER touch _isBranch nodes or their subtrees
      function collectNonBranch(nid: string) {
        const ch = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === nid);
        ch.forEach((c: any) => {
          if (c.data('_isBranch')) return; // skip branches and all their descendants
          toRemove.merge(c);
          collectNonBranch(c.id());
        });
        toRemoveEdges.merge(cyInstance.edges().filter((e: any) => {
          if (e.data('_seqParent') !== nid) return false;
          const tgt = cyInstance.getElementById(e.data('target'));
          return !tgt.length || !tgt.data('_isBranch');
        }));
      }
      siblings.forEach((sib: any) => { toRemove.merge(sib); collectNonBranch(sib.id()); });
      // Remove edges from parent to auto-siblings
      toRemoveEdges.merge(cyInstance.edges().filter((e: any) => {
        if (e.data('_seqParent') !== parentKey) return false;
        const tgt = cyInstance.getElementById(e.data('target'));
        return tgt.length && !tgt.data('_isBranch') && e.data('target') !== nodeId;
      }));
      cyInstance.remove(toRemoveEdges);
      cyInstance.remove(toRemove);
      for (const k of seqExpanded.keys()) {
        if (cyInstance.getElementById(k).length === 0) seqExpanded.delete(k);
      }

      await toggleSeqExpand(funcName, nodeId);
    });

    // No double-click for CFG — use panel button instead

    // Click any node → show info panel (reset previous selection state)
    cyInstance.on('tap', 'node', async (evt: any) => {
      const data = evt.target.data();

      // If clicking a DIFFERENT node, reset path selection
      if (!selectedNode || selectedNode.id !== data.id) {
        selectedPath = null;
        // Reset CFG block highlights when switching nodes
        cyInstance.nodes('.block').style({ opacity: 1 });
        cyInstance.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
      }

      selectedNode = data;
      branchMenu = null;
      contextMenu = null;

      // Load paths for function nodes
      const funcName = data._type === 'function' ? data.label
        : data._type === 'block' ? data._parentFunc
        : data._type === 'seq-next' ? (data._funcName || data.label)
        : null;

      if (funcName && contract && !funcPaths[funcName]) {
        try {
          funcPaths[funcName] = await getPaths(contract.name, funcName);
          funcPaths = { ...funcPaths };
        } catch {}
      }
    });

    // Click background → deselect everything
    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) {
        selectedNode = null;
        selectedPath = null;
        branchMenu = null;
        cyInstance.nodes('.block').style({ opacity: 1 });
        cyInstance.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
      }
    });

    // Collect all descendants of a node recursively via _seqParent
    function collectAllDescendants(rootId: string) {
      let result = cyInstance.collection();
      const direct = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === rootId);
      direct.forEach((c: any) => {
        result = result.union(c);
        result = result.union(collectAllDescendants(c.id()));
      });
      return result;
    }

    // Drag node → move its children together
    cyInstance.on('drag', 'node', (evt: any) => {
      const node = evt.target;
      const prevX = node.data('_prevX');
      const prevY = node.data('_prevY');
      if (prevX === undefined || prevY === undefined) return;

      const delta = { x: evt.position.x - prevX, y: evt.position.y - prevY };
      node.data('_prevX', evt.position.x);
      node.data('_prevY', evt.position.y);

      const nodeId = node.id();
      const nodeType = node.data('_type');
      let children = cyInstance.collection();

      if (nodeType === 'function') {
        const funcName = node.data('label');
        if (expandedFuncs.has(funcName)) {
          children = children.union(cyInstance.nodes(`[_parentFunc = "${funcName}"]`));
        }
        children = children.union(collectAllDescendants(nodeId));
      } else if (nodeType === 'seq-next') {
        children = collectAllDescendants(nodeId);
      }

      children.forEach((child: any) => {
        const pos = child.position();
        child.position({ x: pos.x + delta.x, y: pos.y + delta.y });
      });
    });

    cyInstance.on('grab', 'node', (evt: any) => {
      const pos = evt.target.position();
      evt.target.data('_prevX', pos.x);
      evt.target.data('_prevY', pos.y);
    });

    cyInstance.on('mouseover', 'node.internal', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseover', 'node.seq-next', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });

    // Right-click → context menu
    // Right-click → context menu
    cyInstance.on('cxttap', 'node', (evt: any) => {
      evt.originalEvent?.preventDefault();
      const data = evt.target.data();
      const rect = cyContainer.getBoundingClientRect();
      const pos = evt.renderedPosition || evt.position;
      contextMenu = {
        x: pos.x + rect.left,
        y: pos.y + rect.top,
        nodeId: data.id,
        funcName: data._type === 'function' ? data.label : (data._parentFunc || data._funcName || data.label),
        nodeType: data._type,
      };
    });

    // Grid follows zoom and pan
    function updateGrid() {
      if (!canvasWrap || !cyInstance) return;
      const zoom = cyInstance.zoom();
      const pan = cyInstance.pan();
      const size = 24 * zoom;
      canvasWrap.style.setProperty('--grid-size', `${size}px`);
      canvasWrap.style.setProperty('--grid-x', `${pan.x % size}px`);
      canvasWrap.style.setProperty('--grid-y', `${pan.y % size}px`);
    }
    cyInstance.on('zoom pan', updateGrid);
    updateGrid();
  }

  async function toggleFuncExpand(funcName: string, anchorNodeId?: string) {
    if (!cyInstance || !contract) return;

    // Find the anchor node: either specified or the original function node
    const parentId = anchorNodeId || `${contract.name}::${funcName}`;

    if (expandedFuncs.has(funcName)) {
      // COLLAPSE: animate children to parent, then remove
      const children = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
      const childEdges = cyInstance.edges(`[_parentFunc = "${funcName}"]`);
      const parentNode = cyInstance.getElementById(parentId);
      const parentPos = parentNode.length ? parentNode.position() : { x: 0, y: 0 };

      children.animate({ position: parentPos, style: { opacity: 0 } }, {
        duration: 250,
        complete: () => {
          cyInstance.remove(childEdges);
          cyInstance.remove(children);
        }
      });

      // Restore all nodes visibility
      cyInstance.nodes().style({ opacity: 1 });
      cyInstance.edges().style({ opacity: 1 });

      expandedFuncs.delete(funcName);
      expandedFuncs = new Set(expandedFuncs);
    } else {
      // EXPAND: fetch CFG and add nodes positioned below the function
      if (!cfgCache[funcName]) {
        cfgCache[funcName] = await getCfg(contract.name, funcName);
      }
      const cfg = cfgCache[funcName];
      const anchorNode = cyInstance.getElementById(parentId);
      const parentPos = anchorNode.length ? anchorNode.position() : { x: 0, y: 0 };

      // First add all nodes at parent position (for animation start)
      const newNodes = cfg.nodes.map(n => ({
        group: 'nodes' as const,
        data: {
          id: `cfg:${funcName}:${n.data.id}`,
          label: n.data.label,
          node_type: n.data.node_type,
          statements: n.data.statements,
          _type: 'block',
          _parentFunc: funcName,
        },
        position: { x: parentPos.x, y: parentPos.y },
        classes: `block block-${n.data.node_type.toLowerCase()}`,
      }));

      const newEdges = cfg.edges.map((e, i) => ({
        group: 'edges' as const,
        data: {
          id: `cfg-edge:${funcName}:${i}`,
          source: `cfg:${funcName}:${e.data.source}`,
          target: `cfg:${funcName}:${e.data.target}`,
          kind: e.data.kind,
          _type: 'cfg-edge',
          _parentFunc: funcName,
        },
        classes: e.data.kind.includes('ConditionalTrue') ? 'cond-true' :
                 e.data.kind.includes('ConditionalFalse') ? 'cond-false' :
                 e.data.kind.includes('LoopBack') ? 'loop-back' : '',
      }));

      // Connect function node to CFG entry
      const entryNode = cfg.nodes.find(n => n.data.node_type === 'Entry');
      if (entryNode) {
        newEdges.push({
          group: 'edges' as const,
          data: {
            id: `cfg-link:${funcName}`,
            source: parentId,
            target: `cfg:${funcName}:${entryNode.data.id}`,
            kind: 'expand',
            _type: 'cfg-edge',
            _parentFunc: funcName,
          },
          classes: 'expand-link',
        });
      }

      cyInstance.add([...newNodes, ...newEdges]);

      // Layout ONLY the CFG nodes using dagre, offset below the parent
      const cfgNodes = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
      const cfgEdges = cyInstance.edges(`[_parentFunc = "${funcName}"]`);
      const cfgElements = cfgNodes.union(cfgEdges);

      const subLayout = cfgElements.layout({
        name: 'dagre',
        rankDir: 'TB',
        nodeSep: 30,
        rankSep: 45,
        animate: false,
        fit: false,
      } as any);
      subLayout.run();
      subLayout.stop();

      // Now offset all CFG nodes to be below the parent function
      const cfgBB = cfgNodes.boundingBox();
      const offsetX = parentPos.x - (cfgBB.x1 + cfgBB.w / 2);
      const offsetY = parentPos.y + 60 - cfgBB.y1;

      cfgNodes.forEach((node: any) => {
        const pos = node.position();
        const targetPos = { x: pos.x + offsetX, y: pos.y + offsetY };
        // Animate from parent position to final position
        node.position(parentPos);
        node.animate({ position: targetPos }, { duration: 350, easing: 'ease-out' });
      });

      // Dim other function nodes (still readable) and hide call edges
      cyInstance.nodes('[_type = "function"]').style({ opacity: 0.55 });
      cyInstance.edges('[_type = "call"]').style({ opacity: 0.1 });
      // Keep the expanded function visible
      cyInstance.getElementById(parentId).style({ opacity: 1 });

      expandedFuncs.add(funcName);
      expandedFuncs = new Set(expandedFuncs);
    }
  }

  let branchCounter = 0;

  function addBranch(parentNodeId: string, parentFuncName: string, branchFuncName: string) {
    if (!cyInstance || !seqTree || !seqAnalysis) return;
    branchMenu = null;

    const parentNode = cyInstance.getElementById(parentNodeId);
    const parentPos = parentNode.position();
    const seqKey = parentNodeId;
    // Unique ID — allows multiple branches of the same function
    const uid = ++branchCounter;
    const nodeId = `${seqKey}→${branchFuncName}:b${uid}`;

    const f = seqTree.functions.find((fn: any) => fn.name === branchFuncName);
    if (!f) return;

    const chainParts = (seqKey.includes('::') ? seqKey.split('::')[1].split('→') : seqKey.split('→')).map((s: string) => s.replace(/:b\d+$/, ''));
    const transition = seqAnalysis.transitions.find(
      (t: any) => t.from === parentFuncName && t.to === branchFuncName
    );
    const fullChain = [...chainParts, branchFuncName];
    const chainTransitions: any[] = [];
    for (let i = 0; i < fullChain.length - 1; i++) {
      const t = seqAnalysis.transitions.find(
        (t: any) => t.from === fullChain[i] && t.to === fullChain[i + 1]
      );
      if (t && (t.conditions_affected.length > 0 || t.shared_state.length > 0)) {
        chainTransitions.push(t);
      }
    }
    const hasConditions = chainTransitions.some((t: any) => t.conditions_affected.length > 0);
    const hasShared = chainTransitions.some((t: any) => t.shared_state.length > 0);

    let label = branchFuncName;
    if (hasConditions) label += ' ⚠';

    // Remove auto-expanded SIBLINGS of the parent node (same level, not children)
    // The parent node's _seqParent tells us which level to clean
    const parentData = parentNode.data();
    const parentParentKey = parentData._seqParent;
    if (parentParentKey) {
      const siblings = cyInstance.nodes().filter((n: any) =>
        n.data('_seqParent') === parentParentKey && n.id() !== parentNodeId && !n.data('_isBranch')
      );
      const removeNodes = cyInstance.collection();
      const removeEdges = cyInstance.collection();
      function collectDescs(nid: string) {
        const ch = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === nid);
        ch.forEach((c: any) => {
          if (c.data('_isBranch')) return; // never touch branches
          removeNodes.merge(c);
          collectDescs(c.id());
        });
        removeEdges.merge(cyInstance.edges().filter((e: any) => {
          if (e.data('_seqParent') !== nid) return false;
          const tgt = cyInstance.getElementById(e.data('target'));
          return !tgt.length || !tgt.data('_isBranch');
        }));
      }
      siblings.forEach((sib: any) => { removeNodes.merge(sib); collectDescs(sib.id()); });
      removeEdges.merge(cyInstance.edges().filter((e: any) => {
        if (e.data('_seqParent') !== parentParentKey) return false;
        const tgt = cyInstance.getElementById(e.data('target'));
        return tgt.length && tgt.id() !== parentNodeId && !tgt.data('_isBranch');
      }));
      cyInstance.remove(removeEdges);
      cyInstance.remove(removeNodes);
      for (const k of seqExpanded.keys()) {
        if (cyInstance.getElementById(k).length === 0) seqExpanded.delete(k);
      }
    }

    // Add the branch node
    cyInstance.add([
      {
        group: 'nodes',
        data: {
          id: nodeId, label, _type: 'seq-next', _seqParent: seqKey,
          pathCount: f.path_count, readOnly: f.read_only,
          _transition: transition, _chainTransitions: chainTransitions, _funcName: branchFuncName,
          _isBranch: true,
        },
        position: { x: parentPos.x, y: parentPos.y },
        classes: `seq-next ${f.read_only ? 'readonly' : ''} ${hasConditions ? 'has-conditions' : ''} ${hasShared ? 'has-shared' : ''}`,
      },
      {
        group: 'edges',
        data: {
          id: `se:${nodeId}`, source: parentNodeId, target: nodeId,
          _seqParent: seqKey, label: hasConditions ? '⚠' : hasShared ? '·' : '',
        },
        classes: `seq-edge ${hasConditions ? 'seq-cond' : ''}`,
      },
    ]);

    // Position the new branch node clearly separated from existing children
    const branchNode = cyInstance.getElementById(nodeId);
    const allChildren = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
    const others = allChildren.not(branchNode);
    const isVertical = seqDirection === 'TB';

    let targetPos: { x: number; y: number };
    if (others.length > 0) {
      const bb = others.boundingBox();
      targetPos = isVertical
        ? { x: bb.x2 + 160, y: parentPos.y + 70 }
        : { x: parentPos.x + 160, y: bb.y2 + 60 };
    } else {
      targetPos = isVertical
        ? { x: parentPos.x, y: parentPos.y + 70 }
        : { x: parentPos.x + 160, y: parentPos.y };
    }
    branchNode.animate({ position: targetPos }, { duration: 250, easing: 'ease-out' });
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    if (!cyInstance || !contract || !seqTree) return;
    const seqKey = parentNodeId;
    const parentNode = cyInstance.getElementById(parentNodeId);

    if (seqExpanded.has(seqKey)) {
      // Collapse: remove auto-expanded descendants, keep branches
      const directChildren = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === seqKey);
      const autoChildren = directChildren.filter((n: any) => !n.data('_isBranch'));
      const toRemoveNodes = cyInstance.collection();
      const toRemoveEdges = cyInstance.collection();
      function collectDesc(nid: string) {
        const ch = cyInstance.nodes().filter((n: any) => n.data('_seqParent') === nid);
        ch.forEach((c: any) => {
          if (c.data('_isBranch')) return;
          toRemoveNodes.merge(c);
          collectDesc(c.id());
        });
        toRemoveEdges.merge(cyInstance.edges().filter((e: any) => {
          if (e.data('_seqParent') !== nid) return false;
          const tgt = cyInstance.getElementById(e.data('target'));
          return !tgt.length || !tgt.data('_isBranch');
        }));
      }
      autoChildren.forEach((n: any) => {
        toRemoveNodes.merge(n);
        collectDesc(n.id());
      });
      toRemoveEdges.merge(cyInstance.edges().filter((e: any) => {
        if (e.data('_seqParent') !== seqKey) return false;
        const tgt = cyInstance.getElementById(e.data('target'));
        return tgt.length && !tgt.data('_isBranch');
      }));
      cyInstance.remove(toRemoveEdges);
      cyInstance.remove(toRemoveNodes);
      seqExpanded.delete(seqKey);
      // Clean seqExpanded keys for removed nodes only
      for (const k of seqExpanded.keys()) {
        if (cyInstance.getElementById(k).length === 0) seqExpanded.delete(k);
      }

      const remainingSeq = cyInstance.nodes('[_type = "seq-next"]');
      if (remainingSeq.length === 0) {
        cyInstance.nodes().style({ opacity: 1 });
        cyInstance.edges().style({ opacity: 1 });
      }
      seqExpanded = new Map(seqExpanded);
    } else {
      // Expand: add children below this node. Siblings stay — multiple branches can coexist.
      const parentPos = parentNode.position();
      const funcs = seqTree.functions;
      const newNodes: any[] = [];
      const newEdges: any[] = [];
      const parentFuncName = funcName;

      const chainParts = (seqKey.includes('::') ? seqKey.split('::')[1].split('→') : seqKey.split('→')).map((s: string) => s.replace(/:b\d+$/, ''));

      funcs.forEach((f: any) => {
        const nodeId = `${seqKey}→${f.name}`;
        const readOnly = f.read_only;

        const transition = seqAnalysis?.transitions.find(
          t => t.from === parentFuncName && t.to === f.name
        );

        const fullChain = [...chainParts, f.name];
        const chainTransitions: any[] = [];
        for (let i = 0; i < fullChain.length - 1; i++) {
          const t = seqAnalysis?.transitions.find(
            t => t.from === fullChain[i] && t.to === fullChain[i + 1]
          );
          if (t && (t.conditions_affected.length > 0 || t.shared_state.length > 0)) {
            chainTransitions.push(t);
          }
        }

        const hasConditions = chainTransitions.some(t => t.conditions_affected.length > 0);
        const hasShared = chainTransitions.some(t => t.shared_state.length > 0);

        let label = f.name;
        if (hasConditions) label += ' ⚠';

        newNodes.push({
          group: 'nodes',
          data: {
            id: nodeId,
            label,
            _type: 'seq-next',
            _seqParent: seqKey,
            pathCount: f.path_count,
            readOnly,
            _transition: transition,
            _chainTransitions: chainTransitions,
            _funcName: f.name,
          },
          position: { x: parentPos.x, y: parentPos.y },
          classes: `seq-next ${readOnly ? 'readonly' : ''} ${hasConditions ? 'has-conditions' : ''} ${hasShared ? 'has-shared' : ''}`,
        });

        const edgeLabel = hasConditions ? '⚠' : hasShared ? '·' : '';
        newEdges.push({
          group: 'edges',
          data: {
            id: `se:${seqKey}→${f.name}`,
            source: parentNodeId,
            target: nodeId,
            _seqParent: seqKey,
            label: edgeLabel,
          },
          classes: `seq-edge ${hasConditions ? 'seq-cond' : ''}`,
        });
      });

      // Dim original function nodes
      cyInstance.nodes('[_type = "function"]').style({ opacity: 0.55 });
      cyInstance.edges('[_type = "call"]').style({ opacity: 0.1 });
      cyInstance.getElementById(parentNodeId).style({ opacity: 1 });

      cyInstance.add([...newNodes, ...newEdges]);

      // Make only the NEW nodes bright (don't touch hidden siblings)
      const justAdded = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
      justAdded.style({ opacity: 1 });
      cyInstance.edges(`[_seqParent = "${seqKey}"]`).style({ opacity: 1 });

      // Layout ONLY the new children
      const seqNodes = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
      const seqEdges = cyInstance.edges(`[_seqParent = "${seqKey}"]`);
      const seqElements = seqNodes.union(seqEdges);
      const isVertical = seqDirection === 'TB';

      const subLayout = seqElements.layout({
        name: 'dagre',
        rankDir: seqDirection,
        nodeSep: isVertical ? 30 : 15,
        rankSep: isVertical ? 60 : 50,
        animate: false,
        fit: false,
      } as any);
      subLayout.run();
      subLayout.stop();

      // Position children relative to parent, centered
      const bb = seqNodes.boundingBox();
      let offsetX: number, offsetY: number;
      if (isVertical) {
        offsetX = parentPos.x - bb.x1 - bb.w / 2;
        offsetY = parentPos.y + 70 - bb.y1;
      } else {
        offsetX = parentPos.x + 160 - bb.x1;
        offsetY = parentPos.y - bb.y1 - bb.h / 2;
      }

      seqNodes.forEach((node: any) => {
        const pos = node.position();
        const targetPos = { x: pos.x + offsetX, y: pos.y + offsetY };
        node.position(parentPos);
        node.animate({ position: targetPos }, { duration: 300, easing: 'ease-out' });
      });

      seqExpanded.set(seqKey, true);
      seqExpanded = new Map(seqExpanded);
    }
  }

  function switchMode(newMode: 'cfg' | 'sequences') {
    // Clear all expanded states when switching modes
    if (cyInstance) {
      // Remove all CFG blocks
      const cfgNodes = cyInstance.nodes('[_type = "block"]');
      const cfgEdges = cyInstance.edges('[_type = "cfg-edge"]');
      cyInstance.remove(cfgEdges);
      cyInstance.remove(cfgNodes);
      // Remove all seq nodes
      const seqNodes = cyInstance.nodes('[_type = "seq-next"]');
      const seqEdges = cyInstance.edges('.seq-edge');
      cyInstance.remove(seqEdges);
      cyInstance.remove(seqNodes);
      // Restore opacity
      cyInstance.nodes().style({ opacity: 1 });
      cyInstance.edges().style({ opacity: 1 });
    }
    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    mode = newMode;
  }

  function highlightPath(funcName: string, path: any) {
    if (!cyInstance) return;
    selectedPath = path;

    // Dim all CFG blocks of this function
    const allBlocks = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
    const allCfgEdges = cyInstance.edges(`[_parentFunc = "${funcName}"]`);
    allBlocks.style({ opacity: 0.2 });
    allCfgEdges.style({ opacity: 0.1 });

    // Highlight nodes in the selected path
    const blockIds = path.nodes.map((n: any) => `cfg:${funcName}:b${n.block_id}`);
    blockIds.forEach((id: string) => {
      const node = cyInstance.getElementById(id);
      if (node.length) node.style({ opacity: 1 });
    });

    // Highlight edges between consecutive path nodes
    for (let i = 0; i < blockIds.length - 1; i++) {
      const edges = cyInstance.edges(`[source = "${blockIds[i]}"][target = "${blockIds[i + 1]}"]`);
      edges.style({ opacity: 1 });
    }
  }

  function runLayout(animate: boolean) {
    if (!cyInstance) return;
    const layout = cyInstance.layout({
      name: 'dagre',
      rankDir: 'TB',
      nodeSep: 40,
      rankSep: 55,
      animate,
      animationDuration: animate ? 400 : 0,
      animationEasing: 'ease-in-out-quad',
      fit: !animate, // fit only on initial render
      padding: 40,
    } as any);
    layout.run();
    layout.stop();
  }

  // Palette: dark board
  const C = {
    bg: '#121215',        // near-black background
    surface: '#1a1a22',   // node backgrounds
    border: '#252530',    // subtle borders
    borderHi: '#4a6fa5',  // highlighted borders (muted blue)
    text: '#b8c4d4',      // primary text
    textMuted: '#6b7a8d', // secondary text
    accent: '#5b9bd5',    // primary blue (functions, links)
    accentDark: '#3a6b9f',// darker blue
    accentLight: '#8bb8e8',// light blue (readonly, info)
    warn: '#c49a4a',      // muted amber (conditions)
    warnBorder: '#8a6d30',
    danger: '#b05050',    // muted red (revert, external)
    dangerLight: '#c07070',
    ok: '#5a9a6a',        // muted green (return/success only)
    edge: '#363a48',      // edge color
    edgeHi: '#5b9bd5',    // highlighted edge
  };

  function getStyles() {
    return [
      // Function nodes — main contract functions
      {
        selector: 'node.internal',
        style: {
          'background-color': C.surface, 'label': 'data(label)', 'color': C.accentLight,
          'font-size': '12px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '150px', 'height': '40px', 'shape': 'roundrectangle',
          'border-width': 1.5, 'border-color': C.accent,
        }
      },
      {
        selector: 'node.external',
        style: {
          'background-color': C.bg, 'label': 'data(label)', 'color': C.dangerLight,
          'font-size': '11px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '130px', 'height': '34px', 'shape': 'roundrectangle',
          'border-style': 'dashed', 'border-width': 1, 'border-color': C.danger,
        }
      },
      // CFG block nodes
      {
        selector: 'node.block',
        style: {
          'label': 'data(label)', 'color': C.text, 'font-size': '9px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '160px', 'height': '30px', 'shape': 'roundrectangle',
          'background-color': C.surface, 'border-width': 1, 'border-color': C.border,
          'text-max-width': '150px', 'text-wrap': 'ellipsis',
        }
      },
      { selector: 'node.block-entry', style: { 'background-color': C.accentDark, 'border-color': C.accent, 'color': '#dce8f4' } },
      { selector: 'node.block-return', style: { 'background-color': '#2a4a35', 'border-color': C.ok, 'color': '#b8d4c4', 'width': '90px' } },
      { selector: 'node.block-revert', style: { 'background-color': '#3a2020', 'border-color': C.danger, 'color': C.dangerLight, 'width': '90px' } },
      { selector: 'node.block-loopcondition', style: { 'background-color': '#38301e', 'border-color': C.warn, 'color': '#d4c49a', 'shape': 'diamond', 'width': '90px', 'height': '45px' } },
      { selector: 'node:active', style: { 'overlay-opacity': 0 } },
      // Call edges
      {
        selector: 'edge[_type = "call"]',
        style: {
          'width': 1, 'line-color': C.edge, 'target-arrow-color': C.edge,
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.7,
        }
      },
      {
        selector: 'edge[kind = "External"]',
        style: { 'line-color': '#b0505044', 'target-arrow-color': C.danger, 'line-style': 'dashed' }
      },
      // CFG edges
      {
        selector: 'edge[_type = "cfg-edge"]',
        style: {
          'width': 1, 'line-color': C.border, 'target-arrow-color': C.border,
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.6,
        }
      },
      { selector: 'edge.cond-true', style: { 'line-color': '#5a9a6a66', 'target-arrow-color': C.ok, 'label': '✓', 'font-size': '11px', 'color': C.ok } },
      { selector: 'edge.cond-false', style: { 'line-color': '#b0505066', 'target-arrow-color': C.danger, 'label': '✗', 'font-size': '11px', 'color': C.danger } },
      { selector: 'edge.loop-back', style: { 'line-color': C.warn, 'target-arrow-color': C.warn, 'line-style': 'dashed' } },
      { selector: 'edge.expand-link', style: { 'line-color': '#5b9bd544', 'target-arrow-color': C.accent, 'line-style': 'dotted', 'width': 2 } },
      // Sequence nodes
      {
        selector: 'node.seq-next',
        style: {
          'label': 'data(label)', 'color': C.accentLight, 'font-size': '10px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '110px', 'height': '28px', 'shape': 'roundrectangle',
          'background-color': C.surface, 'border-width': 1.5, 'border-color': C.accent,
        }
      },
      { selector: 'node.seq-next.readonly', style: { 'border-color': C.textMuted, 'color': C.textMuted } },
      { selector: 'node.seq-next.has-conditions', style: { 'background-color': '#2e2818', 'border-color': C.warn, 'border-width': 2, 'color': '#d4c49a' } },
      { selector: 'node.seq-next.has-shared', style: { 'border-style': 'dashed' } },
      {
        selector: 'edge.seq-edge',
        style: {
          'width': 1.5, 'line-color': C.edge, 'target-arrow-color': C.textMuted,
          'target-arrow-shape': 'triangle', 'curve-style': 'straight', 'arrow-scale': 0.7,
        }
      },
      {
        selector: 'edge.seq-cond',
        style: {
          'line-color': C.warn, 'target-arrow-color': C.warn, 'width': 2,
          'label': 'data(label)', 'font-size': '14px', 'color': C.warn,
        }
      },
    ];
  }

  function termColor(t: string): string {
    return t === 'Return' ? C.ok : t === 'Revert' ? C.danger : C.textMuted;
  }
</script>

<div class="view">
  {#if error}
    <div class="error">{error}</div>
  {:else}
    <!-- Floating toolbar -->
    <div class="float-toolbar" style="left:{toolbarX}px;top:{toolbarY}px" onmousedown={onToolbarDown}>
      <a href="/" class="ft-back" title="Back to contracts">←</a>
      <span class="ft-name">{contract?.name ?? '...'}</span>
      <span class="ft-sep"></span>
      <button class="ft-btn" class:active={mode === 'cfg'} onclick={() => switchMode('cfg')}>CFG</button>
      <button class="ft-btn" class:active={mode === 'sequences'} onclick={() => switchMode('sequences')}>Seq</button>
      {#if mode === 'sequences'}
        <span class="ft-sep"></span>
        <button class="ft-btn" class:active={seqDirection === 'TB'} onclick={() => { seqDirection = 'TB'; }} title="Vertical">↓</button>
        <button class="ft-btn" class:active={seqDirection === 'LR'} onclick={() => { seqDirection = 'LR'; }} title="Horizontal">→</button>
      {/if}
      <span class="ft-sep"></span>
      <button class="ft-btn" onclick={toggleSearch} title="Cmd+K">Search</button>
      <button class="ft-btn" onclick={() => { if (cyInstance) cyInstance.fit(undefined, 40); }} title="Center all nodes">Center</button>
    </div>
    <div class="workspace">
      <!-- Sidebar with functions -->
      <div class="sidebar" class:collapsed={!sidebarOpen}>
        <div class="sidebar-header">
          <span class="sidebar-title">Functions</span>
          <button class="sidebar-toggle" onclick={() => sidebarOpen = !sidebarOpen}>{sidebarOpen ? '◂' : '▸'}</button>
        </div>
        {#if sidebarOpen && contract}
          <div class="sidebar-body">
            {#each contract.functions as func}
              {@const onCanvas = canvasFuncs.has(func.name)}
              <button
                class="sidebar-func"
                class:on-canvas={onCanvas}
                onclick={() => onCanvas ? removeFuncFromCanvas(func.name) : addFuncToCanvas(func.name)}
                title={onCanvas ? 'Remove from canvas' : 'Add to canvas'}
              >
                <span class="sf-name">{func.name}</span>
                <span class="sf-meta">{func.path_count}p</span>
                {#if onCanvas}<span class="sf-check">✓</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Canvas with grid -->
      <div class="canvas-wrap" bind:this={canvasWrap}>
        <div class="canvas" bind:this={cyContainer} oncontextmenu={(e) => e.preventDefault()}></div>
      </div>
    </div>

    {#if selectedNode && contract}
      <DraggablePanel
        title={selectedNode._funcName || selectedNode.label || ''}
        x={Math.min(window.innerWidth - 320, window.innerWidth - 20)} y={60} width={Math.min(310, window.innerWidth - 40)}
        onclose={() => selectedNode = null}
      >
        <div class="detail">
          {#if selectedNode._type === 'function'}
            <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.is_external ? 'External' : 'Internal'}</span></div>
            {#if !selectedNode.is_external}
              <div class="d-actions">
                <button class="d-action-btn" onclick={() => toggleFuncExpand(selectedNode.label, selectedNode.id)}>
                  {expandedFuncs.has(selectedNode.label) ? '▼ Collapse CFG' : '▶ Expand CFG'}
                </button>
                {#if mode === 'sequences'}
                  <div class="d-hint">Click → expand · Shift+click → branch</div>
                {/if}
              </div>
            {/if}

            {#if funcPaths[selectedNode.label]}
              {@const fp = funcPaths[selectedNode.label]}
              <div class="d-row"><span class="d-label">Paths</span><span>{fp.stats.total_paths} ({fp.stats.happy_paths} return, {fp.stats.revert_paths} revert)</span></div>

              <Collapsible title="Paths" count={fp.stats.total_paths} open={true}>
                {#each fp.paths as path}
                  <button
                    class="d-path"
                    class:d-path-selected={selectedPath?.id === path.id}
                    onclick={() => highlightPath(selectedNode.label, path)}
                  >
                    <span class="pid">#{path.id}</span>
                    <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
                    <span class="pdepth">{path.nodes.length} steps</span>
                    {#if path.annotations.external_calls.length > 0}
                      <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
                    {/if}
                  </button>
                {/each}
              </Collapsible>

              {#if selectedPath}
                {@const ann = selectedPath.annotations}
                {@const pFunc = selectedNode.label}
                <div class="narrative">
                  {#if ann.require_checks.length > 0}
                    <div class="narr-label">Conditions required</div>
                    {#each ann.require_checks as c}
                      <div class="narr-condition">{c}</div>
                    {/each}
                  {/if}

                  <div class="narr-label" style="margin-top:8px">Execution flow</div>
                  <div class="flow-list">
                    {#each selectedPath.nodes as step, i}
                      {@const cyNode = cyInstance?.getElementById(`cfg:${pFunc}:b${step.block_id}`)}
                      {@const stmts = cyNode?.data('statements') || []}
                      {@const kind = cyNode?.data('node_type') || ''}
                      {@const isLast = i === selectedPath.nodes.length - 1}
                      {@const branchKind = typeof step.branch_taken === 'string' ? step.branch_taken : step.branch_taken?.kind || ''}
                      {#if kind === 'Entry'}
                        <div class="flow-step flow-entry">{pFunc}()</div>
                      {:else if kind === 'Return'}
                        <div class="flow-step flow-return">return</div>
                      {:else if kind === 'Revert'}
                        <div class="flow-step flow-revert">revert</div>
                      {:else}
                        {#each stmts as s}
                          {@const isRequire = s.startsWith('require(') || s.startsWith('require (')}
                          {@const isCall = s.includes('.') && s.includes('(') && !isRequire}
                          {@const isWrite = s.includes('=') && !s.includes('==') && !isCall}
                          <div
                            class="flow-step"
                            class:flow-check={isRequire}
                            class:flow-call={isCall}
                            class:flow-write={isWrite}
                          >
                            {#if branchKind === 'True' && isRequire}
                              <span class="flow-badge pass">✓</span>
                            {:else if branchKind === 'False' && isRequire}
                              <span class="flow-badge fail">✗</span>
                            {:else if isCall}
                              <span class="flow-badge call">→</span>
                            {:else if isWrite}
                              <span class="flow-badge write">✏</span>
                            {/if}
                            {s}
                          </div>
                        {/each}
                      {/if}
                      {#if !isLast}
                        <div class="flow-arrow">│</div>
                      {/if}
                    {/each}
                  </div>

                  {#if ann.external_calls.length > 0 || ann.state_writes.length > 0 || ann.events_emitted.length > 0}
                    <Collapsible title="Side effects" count={ann.external_calls.length + ann.state_writes.length + ann.events_emitted.length} open={false}>
                      {#if ann.external_calls.length > 0}
                        <div class="narr-sub">Calls</div>
                        {#each ann.external_calls as c}
                          <div class="pd-item ext">{c.target}.{c.function}()</div>
                        {/each}
                      {/if}
                      {#if ann.state_writes.length > 0}
                        <div class="narr-sub">Writes</div>
                        {#each ann.state_writes as w}
                          <div class="pd-item wr">{w}</div>
                        {/each}
                      {/if}
                      {#if ann.events_emitted.length > 0}
                        <div class="narr-sub">Emits</div>
                        {#each ann.events_emitted as e}
                          <div class="pd-item ev">{e}</div>
                        {/each}
                      {/if}
                    </Collapsible>
                  {/if}
                </div>
              {:else}
                <div class="d-hint">Click a path to see its execution flow</div>
              {/if}
            {/if}
          {:else if selectedNode._type === 'seq-next'}
            {@const nodeId = selectedNode.id || ''}
            {@const pathParts = (nodeId.includes('::') ? nodeId.split('::')[1] : nodeId).split('→').map((s: string) => s.replace(/:b\d+$/, ''))}
            <div class="d-row"><span class="d-label">Function</span><span>{selectedNode._funcName || selectedNode.label}</span></div>
            <div class="d-row"><span class="d-label">Paths</span><span>{selectedNode.pathCount}</span></div>
            <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.readOnly ? 'Read-only (view)' : 'State-changing'}</span></div>
            {#if pathParts.length > 1}
              <div class="d-path-chain">{pathParts.join(' → ')}</div>
            {/if}
            <div class="d-actions">
              <div class="d-hint">Click → expand · Shift+click → branch</div>
              {#if contract?.functions.some(f => f.name === (selectedNode._funcName || selectedNode.label))}
                <button class="d-action-btn" onclick={() => toggleFuncExpand(selectedNode._funcName || selectedNode.label, selectedNode.id)}>
                  ▶ Expand CFG
                </button>
              {/if}
            </div>

            {#if selectedNode._chainTransitions?.length > 0}
              <div class="d-section">Chain conditions ({selectedNode._chainTransitions.length} transitions)</div>
              {#each selectedNode._chainTransitions as t}
                <div class="d-chain-step">{t.from} → {t.to}</div>
                {#each t.conditions_affected as cond}
                  <div class="pd-item check">{cond}</div>
                {/each}
                {#if t.shared_state?.length > 0}
                  <div class="pd-item wr">shared: {t.shared_state.join(', ')}</div>
                {/if}
                {#if t.has_external_in_from}
                  <div class="pd-item ext">{t.from} has external calls</div>
                {/if}
                {#if t.has_external_in_to}
                  <div class="pd-item ext">{t.to} has external calls</div>
                {/if}
              {/each}
            {:else if selectedNode._transition}
              {#if selectedNode._transition.has_external_in_from || selectedNode._transition.has_external_in_to}
                <div class="d-section">External calls</div>
                {#if selectedNode._transition.has_external_in_from}
                  <div class="pd-item ext">Previous function has external calls</div>
                {/if}
                {#if selectedNode._transition.has_external_in_to}
                  <div class="pd-item ext">This function has external calls</div>
                {/if}
              {:else}
                <div class="d-hint" style="color:#484f58">No state dependencies with previous function</div>
              {/if}
            {:else}
              <div class="d-hint" style="color:#484f58">No state dependencies in chain</div>
            {/if}
          {:else if selectedNode._type === 'block'}
            {@const parentFunc = selectedNode._parentFunc || ''}
            {@const paths = funcPaths[parentFunc]?.paths || []}
            {@const passingPaths = paths.filter((p: any) => p.nodes.some((n: any) => `cfg:${parentFunc}:b${n.block_id}` === selectedNode.id))}
            <div class="d-row"><span class="d-label">Function</span><span>{parentFunc}</span></div>
            <div class="d-row"><span class="d-label">Reachable via</span><span>{passingPaths.length} of {paths.length} paths</span></div>

            {#if passingPaths.length > 0}
              <div class="d-section-label">Select a path to explore</div>
              {#each passingPaths as path}
                <button
                  class="d-path"
                  class:d-path-selected={selectedPath?.id === path.id}
                  onclick={() => highlightPath(parentFunc, path)}
                >
                  <span class="pid">#{path.id}</span>
                  <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
                  <span class="pdepth">{path.nodes.length} steps</span>
                  {#if path.annotations.external_calls.length > 0}
                    <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
                  {/if}
                </button>
              {/each}
            {/if}

            {#if selectedPath}
              {@const currentBlockIdx = selectedPath.nodes.findIndex((n: any) => `cfg:${parentFunc}:b${n.block_id}` === selectedNode.id)}
              {@const routeToHere = currentBlockIdx >= 0 ? selectedPath.nodes.slice(0, currentBlockIdx + 1) : []}
              {@const ann = selectedPath.annotations}

              <div class="narrative">
                {#if ann.require_checks.length > 0}
                  <div class="narr-label">Conditions required</div>
                  {#each ann.require_checks as c}
                    <div class="narr-condition">{c}</div>
                  {/each}
                {/if}

                {#if routeToHere.length > 0}
                  <div class="narr-label" style="margin-top:8px">Execution flow</div>
                  <div class="flow-list">
                    {#each routeToHere as step, i}
                      {@const cyNode = cyInstance?.getElementById(`cfg:${parentFunc}:b${step.block_id}`)}
                      {@const stmts = cyNode?.data('statements') || []}
                      {@const kind = cyNode?.data('node_type') || ''}
                      {@const isHere = i === routeToHere.length - 1}
                      {@const branchKind = typeof step.branch_taken === 'string' ? step.branch_taken : step.branch_taken?.kind || ''}
                      {#if kind === 'Entry'}
                        <div class="flow-step flow-entry">{parentFunc}()</div>
                      {:else if kind === 'Return'}
                        <div class="flow-step flow-return" class:flow-here={isHere}>return {isHere ? '← here' : ''}</div>
                      {:else if kind === 'Revert'}
                        <div class="flow-step flow-revert" class:flow-here={isHere}>revert {isHere ? '← here' : ''}</div>
                      {:else}
                        {#each stmts as s}
                          {@const isRequire = s.startsWith('require(') || s.startsWith('require (')}
                          {@const isCall = s.includes('.') && s.includes('(') && !isRequire}
                          {@const isWrite = s.includes('=') && !s.includes('==') && !isCall}
                          <div
                            class="flow-step"
                            class:flow-check={isRequire}
                            class:flow-call={isCall}
                            class:flow-write={isWrite}
                            class:flow-here={isHere}
                          >
                            {#if branchKind === 'True' && isRequire}
                              <span class="flow-badge pass">✓</span>
                            {:else if branchKind === 'False' && isRequire}
                              <span class="flow-badge fail">✗</span>
                            {:else if isCall}
                              <span class="flow-badge call">→</span>
                            {:else if isWrite}
                              <span class="flow-badge write">✏</span>
                            {/if}
                            {s}
                            {#if isHere}<span class="flow-here-tag">← here</span>{/if}
                          </div>
                        {/each}
                      {/if}
                      {#if !isHere && i < routeToHere.length - 1}
                        <div class="flow-arrow">│</div>
                      {/if}
                    {/each}
                  </div>
                {/if}

                {#if ann.external_calls.length > 0 || ann.state_writes.length > 0 || ann.events_emitted.length > 0}
                  <Collapsible title="Side effects" count={ann.external_calls.length + ann.state_writes.length + ann.events_emitted.length} open={false}>
                    {#if ann.external_calls.length > 0}
                      <div class="narr-sub">Calls</div>
                      {#each ann.external_calls as c}
                        <div class="pd-item ext">{c.target}.{c.function}()</div>
                      {/each}
                    {/if}
                    {#if ann.state_writes.length > 0}
                      <div class="narr-sub">Writes</div>
                      {#each ann.state_writes as w}
                        <div class="pd-item wr">{w}</div>
                      {/each}
                    {/if}
                    {#if ann.events_emitted.length > 0}
                      <div class="narr-sub">Emits</div>
                      {#each ann.events_emitted as e}
                        <div class="pd-item ev">{e}</div>
                      {/each}
                    {/if}
                  </Collapsible>
                {/if}
              </div>
            {:else}
              <div class="d-hint">Click a path above to see the execution flow</div>
            {/if}
          {/if}
        </div>
      </DraggablePanel>
    {/if}

    {#if branchMenu && seqTree}
      <div class="branch-menu" style="left:{branchMenu.x}px;top:{branchMenu.y}px">
        <div class="branch-title">Branch from {branchMenu.parentFuncName}</div>
        {#each seqTree.functions as f}
          <button class="branch-item" onclick={() => addBranch(branchMenu!.parentNodeId, branchMenu!.parentFuncName, f.name)}>
            {f.name}
            {#if f.read_only}<span class="branch-tag">view</span>{/if}
          </button>
        {/each}
        <button class="branch-close" onclick={() => branchMenu = null}>Cancel</button>
      </div>
    {/if}

    {#if contextMenu}
      <div class="ctx-menu" style="left:{contextMenu.x}px;top:{contextMenu.y}px">
        {#if contextMenu.nodeType === 'function'}
          <button class="ctx-item" onclick={() => { toggleFuncExpand(contextMenu!.funcName, contextMenu!.nodeId); contextMenu = null; }}>
            {expandedFuncs.has(contextMenu.funcName) ? '▼ Collapse CFG' : '▶ Expand CFG'}
          </button>
          {#if mode === 'sequences'}
            <button class="ctx-item" onclick={() => { branchMenu = { x: contextMenu!.x, y: contextMenu!.y, parentNodeId: contextMenu!.nodeId, parentFuncName: contextMenu!.funcName }; contextMenu = null; }}>
              + Add branch
            </button>
          {/if}
          <button class="ctx-item ctx-danger" onclick={() => { removeFuncFromCanvas(contextMenu!.funcName); contextMenu = null; selectedNode = null; }}>
            ✕ Remove from canvas
          </button>
        {:else if contextMenu.nodeType === 'seq-next'}
          <button class="ctx-item" onclick={() => { branchMenu = { x: contextMenu!.x, y: contextMenu!.y, parentNodeId: contextMenu!.nodeId, parentFuncName: contextMenu!.funcName }; contextMenu = null; }}>
            + Add branch
          </button>
          {#if seqExpanded.has(contextMenu.nodeId)}
            <button class="ctx-item" onclick={() => { toggleSeqExpand(contextMenu!.funcName, contextMenu!.nodeId); contextMenu = null; }}>
              ▼ Collapse
            </button>
          {/if}
          <button class="ctx-item ctx-danger" onclick={() => { removeSeqNode(contextMenu!.nodeId); contextMenu = null; selectedNode = null; }}>
            ✕ Remove node
          </button>
        {:else if contextMenu.nodeType === 'block'}
          <button class="ctx-item" onclick={() => { if (contextMenu?.funcName) { toggleFuncExpand(contextMenu.funcName); } contextMenu = null; }}>
            ▼ Collapse CFG
          </button>
          <button class="ctx-item ctx-danger" onclick={() => { if (contextMenu?.funcName) { removeFuncFromCanvas(contextMenu.funcName); } contextMenu = null; selectedNode = null; }}>
            ✕ Remove function
          </button>
        {/if}
        <button class="ctx-item" onclick={() => contextMenu = null}>Cancel</button>
      </div>
    {/if}

    <div class="legend">
      {#if mode === 'cfg'}
        <span><span class="dot" style="background:#5b9bd5"></span>Function</span>
        <span><span class="dot" style="background:#3a6b9f"></span>Entry block</span>
        <span><span class="dot" style="background:#5a9a6a"></span>Return</span>
        <span><span class="dot" style="background:#b05050"></span>Revert</span>
        <span>Click → expand CFG</span>
      {:else}
        <span><span class="dot" style="background:#5b9bd5"></span>State-changing</span>
        <span><span class="dot" style="border:1px solid #6b7a8d;background:transparent"></span>Read-only</span>
        <span><span class="dot" style="background:#c49a4a"></span>Conditions affected</span>
        <span>Click → expand · Shift+click → add branch</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #121215; }

  /* Floating toolbar */
  .float-toolbar {
    position: fixed; z-index: 20;
    display: flex; align-items: center; gap: 3px;
    padding: 5px 10px;
    background: #18181eee; border: 1px solid #252530;
    border-radius: 8px; cursor: grab; user-select: none;
    box-shadow: 0 4px 20px #08080a66; backdrop-filter: blur(12px);
  }
  .float-toolbar:active { cursor: grabbing; }
  .ft-back { color: #6b7a8d; text-decoration: none; font-size: 14px; padding: 3px 6px; border-radius: 4px; }
  .ft-back:hover { background: #252530; color: #8bb8e8; }
  .ft-name { font-size: 13px; font-weight: 700; color: #b8c4d4; padding: 0 4px; }
  .ft-sep { width: 1px; height: 16px; background: #252530; margin: 0 2px; }
  .ft-btn { background: none; border: 1px solid transparent; color: #6b7a8d; padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 11px; }
  .ft-btn:hover { border-color: #5b9bd5; color: #8bb8e8; }
  .ft-btn.active { background: #3a6b9f; border-color: #5b9bd5; color: #dce8f4; }

  /* Context menu */
  .ctx-menu {
    position: fixed; z-index: 60;
    background: #18181e; border: 1px solid #252530;
    border-radius: 8px; padding: 4px; min-width: 160px;
    box-shadow: 0 8px 32px #08080a88; backdrop-filter: blur(12px);
  }
  .ctx-item {
    display: block; width: 100%; padding: 6px 10px;
    background: none; border: none; color: #b8c4d4;
    font-size: 12px; cursor: pointer; border-radius: 4px;
    text-align: left; font-family: inherit;
  }
  .ctx-item:hover { background: #1e1e28; color: #8bb8e8; }
  .ctx-item.ctx-danger:hover { background: #b0505015; color: #b05050; }

  .error { padding: 24px; color: #b05050; }

  .workspace { flex: 1; display: flex; overflow: hidden; height: 100%; }

  /* Sidebar */
  .sidebar {
    width: 180px; flex-shrink: 0;
    background: #18181e; border-right: 1px solid #252530;
    display: flex; flex-direction: column;
    transition: width 0.2s;
  }
  .sidebar.collapsed { width: 32px; }
  .sidebar-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 8px; border-bottom: 1px solid #252530;
  }
  .sidebar-title { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .collapsed .sidebar-title { display: none; }
  .sidebar-toggle {
    background: none; border: none; color: #6b7a8d; cursor: pointer;
    font-size: 11px; padding: 2px 4px;
  }
  .sidebar-toggle:hover { color: #8bb8e8; }
  .sidebar-body { flex: 1; overflow-y: auto; padding: 4px; }
  .sidebar-func {
    display: flex; align-items: center; gap: 4px; width: 100%;
    padding: 5px 6px; background: none; border: none;
    color: #6b7a8d; font-size: 11px; font-family: monospace;
    cursor: pointer; border-radius: 4px; text-align: left;
  }
  .sidebar-func:hover { background: #1e1e28; color: #b8c4d4; }
  .sidebar-func.on-canvas { color: #8bb8e8; }
  .sf-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sf-meta { font-size: 9px; color: #4a5568; }
  .sf-check { color: #5b9bd5; font-size: 10px; }

  /* Canvas with dot grid that follows zoom/pan */
  .canvas-wrap {
    flex: 1; position: relative;
    --grid-size: 24px; --grid-x: 0px; --grid-y: 0px;
  }
  .canvas-wrap::before {
    content: '';
    position: absolute; inset: 0; z-index: 0; pointer-events: none;
    background-image: radial-gradient(circle, #333340 1px, transparent 1px);
    background-size: var(--grid-size) var(--grid-size);
    background-position: var(--grid-x) var(--grid-y);
  }
  .canvas { position: absolute; inset: 0; z-index: 1; }

  .detail { padding: 8px; }
  .d-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 12px; color: #b8c4d4; }
  .d-label { color: #6b7a8d; }
  .d-hint { font-size: 11px; color: #5b9bd5; padding: 6px 0; font-style: italic; }
  .d-actions { padding: 6px 0; display: flex; flex-direction: column; gap: 4px; }
  .d-action-btn {
    background: #1a1a22; border: 1px solid #252530; color: #8bb8e8;
    padding: 6px 10px; border-radius: 4px; cursor: pointer;
    font-size: 11px; font-family: monospace; text-align: left;
  }
  .d-action-btn:hover { border-color: #5b9bd5; background: #1e1e28; }
  .d-section { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; font-weight: 600; }
  .d-chain-step { font-size: 11px; color: #8bb8e8; font-weight: 600; margin: 6px 0 2px; padding-top: 4px; border-top: 1px solid #2a2d38; }
  .d-path-chain { font-size: 10px; color: #4a5568; padding: 4px 0; font-family: monospace; word-break: break-all; }

  .d-section-label { font-size: 10px; color: #4a5568; margin: 8px 0 4px; }

  /* Narrative panel */
  .narrative { margin-top: 6px; }
  .narr-label { font-size: 10px; color: #6b7a8d; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 4px; }
  .narr-sub { font-size: 9px; color: #4a5568; text-transform: uppercase; margin: 6px 0 2px; }
  .narr-condition {
    font-family: monospace; font-size: 11px;
    padding: 3px 8px; margin: 2px 0;
    background: #c49a4a12; border-left: 2px solid #c49a4a;
    color: #c49a4a; border-radius: 0 3px 3px 0;
  }

  /* Flow list */
  .flow-list { display: flex; flex-direction: column; gap: 0; }
  .flow-arrow { color: #252530; font-size: 10px; padding-left: 6px; line-height: 1; }
  .flow-step {
    font-family: monospace; font-size: 11px; color: #b8c4d4;
    padding: 4px 8px; border-radius: 4px;
    display: flex; align-items: center; gap: 5px;
    border-left: 2px solid #252530;
  }
  .flow-step.flow-entry { color: #8bb8e8; font-weight: 600; border-left-color: #5b9bd5; }
  .flow-step.flow-return { color: #5a9a6a; border-left-color: #5a9a6a; }
  .flow-step.flow-revert { color: #b05050; border-left-color: #b05050; }
  .flow-step.flow-check { color: #c49a4a; border-left-color: #c49a4a; background: #c49a4a08; }
  .flow-step.flow-call { color: #b8c4d4; border-left-color: #b05050; }
  .flow-step.flow-write { color: #6b7a8d; border-left-color: #5b9bd5; }
  .flow-step.flow-here { background: #5b9bd512; border-left-color: #5b9bd5; }
  .flow-badge {
    font-size: 9px; width: 14px; height: 14px;
    display: inline-flex; align-items: center; justify-content: center;
    border-radius: 3px; flex-shrink: 0;
  }
  .flow-badge.pass { background: #5a9a6a22; color: #5a9a6a; }
  .flow-badge.fail { background: #b0505022; color: #b05050; }
  .flow-badge.call { background: #b0505015; color: #b05050; }
  .flow-badge.write { background: #5b9bd515; color: #5b9bd5; }
  .flow-here-tag { color: #5b9bd5; font-size: 9px; margin-left: auto; }
  .d-path { display: flex; align-items: center; gap: 4px; padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit; background: transparent; border: 1px solid transparent; cursor: pointer; width: 100%; text-align: left; font: inherit; }
  .d-path:hover { background: #121215; }
  .pid { color: #4a5568; font-weight: 600; }
  .pdepth { color: #4a5568; font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: #b0505018; color: #c07070; }
  .pb.wr { background: #5b9bd518; color: #8bb8e8; }
  .d-path-selected { background: #1e2028; border-color: #5b9bd5; }

  .path-detail-inline {
    margin-top: 8px; padding-top: 8px;
    border-top: 1px solid #2a2d38;
  }
  .pd-title { font-size: 9px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; margin: 6px 0 2px; }
  .pd-item {
    font-family: monospace; font-size: 11px;
    padding: 2px 6px; border-radius: 3px; margin-bottom: 2px;
  }
  .pd-item.check { background: #c49a4a18; color: #c49a4a; }
  .pd-item.ext { background: #b0505018; color: #c07070; }
  .pd-item.wr { background: #5b9bd518; color: #8bb8e8; }
  .pd-item.ev { background: #5a9a6a18; color: #7aba8a; }
  .d-stmt { font-family: monospace; font-size: 11px; padding: 3px 6px; background: #121215; border-radius: 3px; margin-bottom: 2px; color: #b8c4d4; }


  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px; font-size: 11px; color: #6b7a8d;
    background: #18181edd; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #252530; z-index: 10;
    backdrop-filter: blur(8px);
  }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }

  .branch-menu {
    position: fixed; z-index: 60;
    background: #18181e; border: 1px solid #252530;
    border-radius: 8px; padding: 4px;
    box-shadow: 0 8px 32px #08080a88;
    backdrop-filter: blur(12px);
    max-height: 280px; overflow-y: auto;
    min-width: 160px;
  }
  .branch-title { font-size: 10px; color: #6b7a8d; padding: 4px 8px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }
  .branch-item {
    display: flex; align-items: center; gap: 6px; width: 100%;
    padding: 6px 10px; background: none; border: none;
    color: #b8c4d4; font-size: 12px; font-family: monospace;
    cursor: pointer; border-radius: 4px; text-align: left;
  }
  .branch-item:hover { background: #1e1e28; color: #8bb8e8; }
  .branch-tag { font-size: 9px; color: #6b7a8d; background: #252830; padding: 1px 5px; border-radius: 8px; }
  .branch-close {
    display: block; width: 100%; padding: 5px 10px; margin-top: 2px;
    background: none; border: none; border-top: 1px solid #2a2d38;
    color: #4a5568; font-size: 11px; cursor: pointer; text-align: center;
  }
  .branch-close:hover { color: #b05050; }
</style>
