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
  import GraphCanvas from '$lib/components/contract/GraphCanvas.svelte';

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

  let graphCanvas: GraphCanvas;
  let callgraphData: CytoscapeGraph | null = $state(null);
  let cfgCache: Record<string, CytoscapeGraph> = {};

  onMount(async () => {
    const contractName = page.params.name;
    if (!contractName) return;
    setSearchContext(contractName);
    try {
      contract = await getContract(contractName);
      callgraphData = await getCallGraph(contractName);
      try { seqTree = await getSequences(contractName); } catch {}
      try { seqAnalysis = await getSequenceAnalysis(contractName); } catch {}
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  // Listen for search result navigation
  $effect(() => {
    const nav = getSearchNavigate();
    if (!nav || !graphCanvas?.getCy() || !contract) return;
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

      const funcNode = graphCanvas?.getCy().nodes().filter((n: any) =>
        n.data('label') === nav.func && n.data('_type') === 'function'
      );
      if (funcNode.length) {
        selectedNode = funcNode.data();
        const path = funcPaths[nav.func]?.paths?.find((p: any) => p.id === nav.pathId);
        if (path) highlightPath(nav.func, path);
        graphCanvas?.getCy().animate({ center: { eles: funcNode }, zoom: graphCanvas?.getCy().zoom() }, { duration: 300 });
      }

      if (!stale) setSearchNavigate(null);
    })();

    return () => { stale = true; };
  });

  function addFuncToCanvas(funcName: string) {
    if (!graphCanvas?.getCy() || !callgraphData || canvasFuncs.has(funcName)) return;
    const nodeData = callgraphData.nodes.find(n => n.data.label === funcName);
    if (!nodeData) return;

    const center = graphCanvas?.getCy().extent();
    const x = (center.x1 + center.x2) / 2 + (canvasFuncs.size % 3 - 1) * 180;
    const y = (center.y1 + center.y2) / 2 + Math.floor(canvasFuncs.size / 3) * 70;

    graphCanvas?.getCy().add({
      group: 'nodes',
      data: { ...nodeData.data, _type: 'function' },
      classes: nodeData.data.is_external ? 'external' : 'internal',
      position: { x, y },
    });

    canvasFuncs.add(funcName);
    canvasFuncs = new Set(canvasFuncs);
  }

  function removeSeqNode(nodeId: string) {
    if (!graphCanvas?.getCy()) return;
    const node = graphCanvas?.getCy().getElementById(nodeId);
    if (!node.length) return;
    // Remove all descendants recursively
    const toRemove = graphCanvas?.getCy().collection();
    const toRemoveEdges = graphCanvas?.getCy().collection();
    function collect(nid: string) {
      const ch = graphCanvas?.getCy().nodes().filter((n: any) => n.data('_seqParent') === nid);
      ch.forEach((c: any) => { toRemove.merge(c); collect(c.id()); });
      toRemoveEdges.merge(graphCanvas?.getCy().edges().filter((e: any) => e.data('_seqParent') === nid));
    }
    collect(nodeId);
    // Also remove edges pointing to this node
    toRemoveEdges.merge(node.connectedEdges());
    toRemove.merge(node);
    graphCanvas?.getCy().remove(toRemoveEdges);
    graphCanvas?.getCy().remove(toRemove);
    seqExpanded.delete(nodeId);
    for (const k of seqExpanded.keys()) {
      if (graphCanvas?.getCy().getElementById(k).length === 0) seqExpanded.delete(k);
    }
  }

  function removeFuncFromCanvas(funcName: string) {
    if (!graphCanvas?.getCy() || !canvasFuncs.has(funcName)) return;
    const node = graphCanvas?.getCy().nodes().filter((n: any) => n.data('label') === funcName && n.data('_type') === 'function');
    if (!node.length) return;
    const nodeId = node.id();
    // Remove all descendants
    const desc = graphCanvas?.getCy().nodes().filter((n: any) => {
      const sp = n.data('_seqParent');
      return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
    });
    const descEdges = graphCanvas?.getCy().edges().filter((e: any) => {
      const sp = e.data('_seqParent');
      return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
    });
    graphCanvas?.getCy().remove(descEdges);
    graphCanvas?.getCy().remove(desc);
    // Remove CFG children
    graphCanvas?.getCy().remove(graphCanvas?.getCy().nodes(`[_parentFunc = "${funcName}"]`));
    graphCanvas?.getCy().remove(graphCanvas?.getCy().edges(`[_parentFunc = "${funcName}"]`));
    // Remove the node itself and its call edges
    graphCanvas?.getCy().remove(node.connectedEdges());
    graphCanvas?.getCy().remove(node);
    canvasFuncs.delete(funcName);
    canvasFuncs = new Set(canvasFuncs);
    expandedFuncs.delete(funcName);
    seqExpanded.delete(nodeId);
  }

  // canvasFuncs reset when callgraphData is loaded (GraphCanvas will init from it)
  $effect(() => {
    if (callgraphData) {
      canvasFuncs = new Set();
    }
  });

  // --- GraphCanvas event handlers ---

  async function handleNodeTap(data: any) {
    const cy = graphCanvas?.getCy();
    if (!cy) return;

    // If clicking a DIFFERENT node, reset path selection
    if (!selectedNode || selectedNode.id !== data.id) {
      selectedPath = null;
      cy.nodes('.block').style({ opacity: 1 });
      cy.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
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
  }

  function handleBackgroundTap() {
    const cy = graphCanvas?.getCy();
    selectedNode = null;
    selectedPath = null;
    branchMenu = null;
    if (cy) {
      cy.nodes('.block').style({ opacity: 1 });
      cy.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
    }
  }

  function handleContextMenu(x: number, y: number, nodeId: string, funcName: string, nodeType: string) {
    contextMenu = { x, y, nodeId, funcName, nodeType };
    branchMenu = null;
  }

  async function handleFunctionTap(funcName: string, nodeId: string, shiftKey: boolean) {
    branchMenu = null;
    if (mode === 'cfg') {
      await toggleFuncExpand(funcName);
    } else if (mode === 'sequences') {
      if (shiftKey) {
        const container = graphCanvas?.getContainer();
        if (!container) return;
        const cy = graphCanvas?.getCy();
        const node = cy?.getElementById(nodeId);
        if (!node?.length) return;
        const rect = container.getBoundingClientRect();
        const pos = node.renderedPosition();
        branchMenu = { x: pos.x + rect.left, y: pos.y + rect.top, parentNodeId: nodeId, parentFuncName: funcName };
      } else {
        await toggleSeqExpand(funcName, nodeId);
      }
    }
  }

  async function handleSeqNodeTap(funcName: string, nodeId: string, shiftKey: boolean, isBranch: boolean, seqParent: string) {
    const cy = graphCanvas?.getCy();
    if (!cy) return;
    branchMenu = null;

    if (shiftKey) {
      const container = graphCanvas?.getContainer();
      if (!container) return;
      const node = cy.getElementById(nodeId);
      if (!node?.length) return;
      const rect = container.getBoundingClientRect();
      const pos = node.renderedPosition();
      branchMenu = { x: pos.x + rect.left, y: pos.y + rect.top, parentNodeId: nodeId, parentFuncName: funcName };
      return;
    }

    if (seqExpanded.has(nodeId)) {
      await toggleSeqExpand(funcName, nodeId);
      return;
    }

    // If this is a branch node, just expand it
    if (isBranch) {
      await toggleSeqExpand(funcName, nodeId);
      return;
    }

    // Auto-expanded node: remove other auto-expanded siblings (keep ALL branches)
    const parentKey = seqParent;
    const siblings = cy.nodes().filter((n: any) => n.data('_seqParent') === parentKey && n.id() !== nodeId && !n.data('_isBranch'));
    const toRemove = cy.collection();
    const toRemoveEdges = cy.collection();
    function collectNonBranch(nid: string) {
      const ch = cy.nodes().filter((n: any) => n.data('_seqParent') === nid);
      ch.forEach((c: any) => {
        if (c.data('_isBranch')) return;
        toRemove.merge(c);
        collectNonBranch(c.id());
      });
      toRemoveEdges.merge(cy.edges().filter((e: any) => {
        if (e.data('_seqParent') !== nid) return false;
        const tgt = cy.getElementById(e.data('target'));
        return !tgt.length || !tgt.data('_isBranch');
      }));
    }
    siblings.forEach((sib: any) => { toRemove.merge(sib); collectNonBranch(sib.id()); });
    toRemoveEdges.merge(cy.edges().filter((e: any) => {
      if (e.data('_seqParent') !== parentKey) return false;
      const tgt = cy.getElementById(e.data('target'));
      return tgt.length && !tgt.data('_isBranch') && e.data('target') !== nodeId;
    }));
    cy.remove(toRemoveEdges);
    cy.remove(toRemove);
    for (const k of seqExpanded.keys()) {
      if (cy.getElementById(k).length === 0) seqExpanded.delete(k);
    }

    await toggleSeqExpand(funcName, nodeId);
  }

  async function toggleFuncExpand(funcName: string, anchorNodeId?: string) {
    if (!graphCanvas?.getCy() || !contract) return;

    // Find the anchor node: either specified or the original function node
    const parentId = anchorNodeId || `${contract.name}::${funcName}`;

    if (expandedFuncs.has(funcName)) {
      // COLLAPSE: animate children to parent, then remove
      const children = graphCanvas?.getCy().nodes(`[_parentFunc = "${funcName}"]`);
      const childEdges = graphCanvas?.getCy().edges(`[_parentFunc = "${funcName}"]`);
      const parentNode = graphCanvas?.getCy().getElementById(parentId);
      const parentPos = parentNode.length ? parentNode.position() : { x: 0, y: 0 };

      children.animate({ position: parentPos, style: { opacity: 0 } }, {
        duration: 250,
        complete: () => {
          graphCanvas?.getCy().remove(childEdges);
          graphCanvas?.getCy().remove(children);
        }
      });

      // Restore all nodes visibility
      graphCanvas?.getCy().nodes().style({ opacity: 1 });
      graphCanvas?.getCy().edges().style({ opacity: 1 });

      expandedFuncs.delete(funcName);
      expandedFuncs = new Set(expandedFuncs);
    } else {
      // EXPAND: fetch CFG and add nodes positioned below the function
      if (!cfgCache[funcName]) {
        cfgCache[funcName] = await getCfg(contract.name, funcName);
      }
      const cfg = cfgCache[funcName];
      const anchorNode = graphCanvas?.getCy().getElementById(parentId);
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

      graphCanvas?.getCy().add([...newNodes, ...newEdges]);

      // Layout ONLY the CFG nodes using dagre, offset below the parent
      const cfgNodes = graphCanvas?.getCy().nodes(`[_parentFunc = "${funcName}"]`);
      const cfgEdges = graphCanvas?.getCy().edges(`[_parentFunc = "${funcName}"]`);
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
      graphCanvas?.getCy().nodes('[_type = "function"]').style({ opacity: 0.55 });
      graphCanvas?.getCy().edges('[_type = "call"]').style({ opacity: 0.1 });
      // Keep the expanded function visible
      graphCanvas?.getCy().getElementById(parentId).style({ opacity: 1 });

      expandedFuncs.add(funcName);
      expandedFuncs = new Set(expandedFuncs);
    }
  }

  let branchCounter = 0;

  function addBranch(parentNodeId: string, parentFuncName: string, branchFuncName: string) {
    if (!graphCanvas?.getCy() || !seqTree || !seqAnalysis) return;
    branchMenu = null;

    const parentNode = graphCanvas?.getCy().getElementById(parentNodeId);
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
      const siblings = graphCanvas?.getCy().nodes().filter((n: any) =>
        n.data('_seqParent') === parentParentKey && n.id() !== parentNodeId && !n.data('_isBranch')
      );
      const removeNodes = graphCanvas?.getCy().collection();
      const removeEdges = graphCanvas?.getCy().collection();
      function collectDescs(nid: string) {
        const ch = graphCanvas?.getCy().nodes().filter((n: any) => n.data('_seqParent') === nid);
        ch.forEach((c: any) => {
          if (c.data('_isBranch')) return; // never touch branches
          removeNodes.merge(c);
          collectDescs(c.id());
        });
        removeEdges.merge(graphCanvas?.getCy().edges().filter((e: any) => {
          if (e.data('_seqParent') !== nid) return false;
          const tgt = graphCanvas?.getCy().getElementById(e.data('target'));
          return !tgt.length || !tgt.data('_isBranch');
        }));
      }
      siblings.forEach((sib: any) => { removeNodes.merge(sib); collectDescs(sib.id()); });
      removeEdges.merge(graphCanvas?.getCy().edges().filter((e: any) => {
        if (e.data('_seqParent') !== parentParentKey) return false;
        const tgt = graphCanvas?.getCy().getElementById(e.data('target'));
        return tgt.length && tgt.id() !== parentNodeId && !tgt.data('_isBranch');
      }));
      graphCanvas?.getCy().remove(removeEdges);
      graphCanvas?.getCy().remove(removeNodes);
      for (const k of seqExpanded.keys()) {
        if (graphCanvas?.getCy().getElementById(k).length === 0) seqExpanded.delete(k);
      }
    }

    // Add the branch node
    graphCanvas?.getCy().add([
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
    const branchNode = graphCanvas?.getCy().getElementById(nodeId);
    const allChildren = graphCanvas?.getCy().nodes(`[_seqParent = "${seqKey}"]`);
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
    if (!graphCanvas?.getCy() || !contract || !seqTree) return;
    const seqKey = parentNodeId;
    const parentNode = graphCanvas?.getCy().getElementById(parentNodeId);

    if (seqExpanded.has(seqKey)) {
      // Collapse: remove auto-expanded descendants, keep branches
      const directChildren = graphCanvas?.getCy().nodes().filter((n: any) => n.data('_seqParent') === seqKey);
      const autoChildren = directChildren.filter((n: any) => !n.data('_isBranch'));
      const toRemoveNodes = graphCanvas?.getCy().collection();
      const toRemoveEdges = graphCanvas?.getCy().collection();
      function collectDesc(nid: string) {
        const ch = graphCanvas?.getCy().nodes().filter((n: any) => n.data('_seqParent') === nid);
        ch.forEach((c: any) => {
          if (c.data('_isBranch')) return;
          toRemoveNodes.merge(c);
          collectDesc(c.id());
        });
        toRemoveEdges.merge(graphCanvas?.getCy().edges().filter((e: any) => {
          if (e.data('_seqParent') !== nid) return false;
          const tgt = graphCanvas?.getCy().getElementById(e.data('target'));
          return !tgt.length || !tgt.data('_isBranch');
        }));
      }
      autoChildren.forEach((n: any) => {
        toRemoveNodes.merge(n);
        collectDesc(n.id());
      });
      toRemoveEdges.merge(graphCanvas?.getCy().edges().filter((e: any) => {
        if (e.data('_seqParent') !== seqKey) return false;
        const tgt = graphCanvas?.getCy().getElementById(e.data('target'));
        return tgt.length && !tgt.data('_isBranch');
      }));
      graphCanvas?.getCy().remove(toRemoveEdges);
      graphCanvas?.getCy().remove(toRemoveNodes);
      seqExpanded.delete(seqKey);
      // Clean seqExpanded keys for removed nodes only
      for (const k of seqExpanded.keys()) {
        if (graphCanvas?.getCy().getElementById(k).length === 0) seqExpanded.delete(k);
      }

      const remainingSeq = graphCanvas?.getCy().nodes('[_type = "seq-next"]');
      if (remainingSeq.length === 0) {
        graphCanvas?.getCy().nodes().style({ opacity: 1 });
        graphCanvas?.getCy().edges().style({ opacity: 1 });
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
      graphCanvas?.getCy().nodes('[_type = "function"]').style({ opacity: 0.55 });
      graphCanvas?.getCy().edges('[_type = "call"]').style({ opacity: 0.1 });
      graphCanvas?.getCy().getElementById(parentNodeId).style({ opacity: 1 });

      graphCanvas?.getCy().add([...newNodes, ...newEdges]);

      // Make only the NEW nodes bright (don't touch hidden siblings)
      const justAdded = graphCanvas?.getCy().nodes(`[_seqParent = "${seqKey}"]`);
      justAdded.style({ opacity: 1 });
      graphCanvas?.getCy().edges(`[_seqParent = "${seqKey}"]`).style({ opacity: 1 });

      // Layout ONLY the new children
      const seqNodes = graphCanvas?.getCy().nodes(`[_seqParent = "${seqKey}"]`);
      const seqEdges = graphCanvas?.getCy().edges(`[_seqParent = "${seqKey}"]`);
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
    if (graphCanvas?.getCy()) {
      // Remove all CFG blocks
      const cfgNodes = graphCanvas?.getCy().nodes('[_type = "block"]');
      const cfgEdges = graphCanvas?.getCy().edges('[_type = "cfg-edge"]');
      graphCanvas?.getCy().remove(cfgEdges);
      graphCanvas?.getCy().remove(cfgNodes);
      // Remove all seq nodes
      const seqNodes = graphCanvas?.getCy().nodes('[_type = "seq-next"]');
      const seqEdges = graphCanvas?.getCy().edges('.seq-edge');
      graphCanvas?.getCy().remove(seqEdges);
      graphCanvas?.getCy().remove(seqNodes);
      // Restore opacity
      graphCanvas?.getCy().nodes().style({ opacity: 1 });
      graphCanvas?.getCy().edges().style({ opacity: 1 });
    }
    expandedFuncs = new Set();
    seqExpanded = new Map();
    selectedNode = null;
    selectedPath = null;
    mode = newMode;
  }

  function highlightPath(funcName: string, path: any) {
    if (!graphCanvas?.getCy()) return;
    selectedPath = path;

    // Dim all CFG blocks of this function
    const allBlocks = graphCanvas?.getCy().nodes(`[_parentFunc = "${funcName}"]`);
    const allCfgEdges = graphCanvas?.getCy().edges(`[_parentFunc = "${funcName}"]`);
    allBlocks.style({ opacity: 0.2 });
    allCfgEdges.style({ opacity: 0.1 });

    // Highlight nodes in the selected path
    const blockIds = path.nodes.map((n: any) => `cfg:${funcName}:b${n.block_id}`);
    blockIds.forEach((id: string) => {
      const node = graphCanvas?.getCy().getElementById(id);
      if (node.length) node.style({ opacity: 1 });
    });

    // Highlight edges between consecutive path nodes
    for (let i = 0; i < blockIds.length - 1; i++) {
      const edges = graphCanvas?.getCy().edges(`[source = "${blockIds[i]}"][target = "${blockIds[i + 1]}"]`);
      edges.style({ opacity: 1 });
    }
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
      oncenter={() => { if (graphCanvas?.getCy()) graphCanvas?.getCy().fit(undefined, 40); }}
      onseqdirection={(dir) => { seqDirection = dir; }}
    />
    <div class="workspace">
      {#if contract}
        <FunctionSidebar {contract} {canvasFuncs} onadd={addFuncToCanvas} onremove={removeFuncFromCanvas} />
      {/if}

      <GraphCanvas
        bind:this={graphCanvas}
        graphData={callgraphData}
        {expandedFuncs}
        onnodetap={handleNodeTap}
        onbackgroundtap={handleBackgroundTap}
        onnodecontextmenu={handleContextMenu}
        onfunctiontap={handleFunctionTap}
        onseqnodetap={handleSeqNodeTap}
      />
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
        cyInstance={graphCanvas?.getCy()}
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
