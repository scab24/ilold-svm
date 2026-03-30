<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, getSequenceAnalysis, type ContractDetail, type CytoscapeGraph, type SequenceAnalysis } from '$lib/api/rest';
  import { toggleSearch, setSearchContext } from '$lib/stores/search';
  import DraggablePanel from '$lib/DraggablePanel.svelte';

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

  let cyContainer: HTMLDivElement;
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
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  });

  onDestroy(() => {
    if (cyInstance) { cyInstance.destroy(); cyInstance = null; }
  });

  async function renderGraph(graph: CytoscapeGraph) {
    const cytoscape = (await import('cytoscape')).default;
    if (!dagreRegistered) {
      const dagre = (await import('cytoscape-dagre')).default;
      cytoscape.use(dagre);
      dagreRegistered = true;
    }
    if (cyInstance) cyInstance.destroy();

    const nodes = graph.nodes
      .filter(n => n.data.label.length > 0)
      .map(n => ({
        group: 'nodes' as const,
        data: { ...n.data, _type: 'function' },
        classes: n.data.is_external ? 'external' : 'internal',
      }));
    const nodeIds = new Set(nodes.map(n => n.data.id));
    const edges = graph.edges
      .filter(e => nodeIds.has(e.data.source) && nodeIds.has(e.data.target))
      .map(e => ({ group: 'edges' as const, data: { ...e.data, _type: 'call' } }));

    cyInstance = cytoscape({
      container: cyContainer,
      elements: [...nodes, ...edges],
      style: getStyles() as any,
      layout: { name: 'preset' },
      minZoom: 0.1, maxZoom: 5, wheelSensitivity: 0.3,
    });

    runLayout(false);

    // Single click on function nodes
    cyInstance.on('tap', 'node.internal', async (evt: any) => {
      const data = evt.target.data();
      if (data._type !== 'function') return;
      branchMenu = null;
      if (mode === 'cfg') {
        await toggleFuncExpand(data.label);
      } else if (mode === 'sequences') {
        if (evt.originalEvent?.shiftKey && seqExpanded.has(data.id)) {
          // Shift+click on expanded node → show branch menu
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

      // Auto-expanded node: remove other auto-expanded siblings (keep branches)
      const parentKey = data._seqParent;
      const me = cyInstance.getElementById(nodeId);
      const siblings = cyInstance.nodes(`[_seqParent = "${parentKey}"]`).not(me).filter((n: any) => !n.data('_isBranch'));
      const toRemove = cyInstance.collection();
      const toRemoveEdges = cyInstance.collection();
      function collectAll(nid: string) {
        const ch = cyInstance.nodes(`[_seqParent = "${nid}"]`);
        ch.forEach((c: any) => { toRemove.merge(c); collectAll(c.id()); });
        toRemoveEdges.merge(cyInstance.edges().filter((e: any) => e.data('_seqParent') === nid));
      }
      siblings.forEach((sib: any) => { toRemove.merge(sib); collectAll(sib.id()); });
      const sibEdges = cyInstance.edges(`[_seqParent = "${parentKey}"]`).filter((e: any) => {
        const tgt = cyInstance.getElementById(e.data('target'));
        return tgt.length && !tgt.data('_isBranch') && e.data('target') !== nodeId;
      });
      toRemoveEdges.merge(sibEdges);
      cyInstance.remove(toRemoveEdges);
      cyInstance.remove(toRemove);
      for (const k of seqExpanded.keys()) {
        if (cyInstance.getElementById(k).length === 0) seqExpanded.delete(k);
      }

      await toggleSeqExpand(funcName, nodeId);
    });

    // Double click on ANY function node (original or seq-next) → expand its CFG
    cyInstance.on('dbltap', 'node', async (evt: any) => {
      const data = evt.target.data();
      const funcName = data.label;
      const nodeId = data.id;
      if (!funcName) return;
      if (contract?.functions.some(f => f.name === funcName)) {
        await toggleFuncExpand(funcName, nodeId);
      }
    });

    // Click any node → show info
    cyInstance.on('tap', 'node', async (evt: any) => {
      const data = evt.target.data();
      selectedNode = data;

      if (data._type === 'function' && data.label && contract && !funcPaths[data.label]) {
        try {
          funcPaths[data.label] = await getPaths(contract.name, data.label);
          funcPaths = { ...funcPaths };
        } catch {}
      }
    });

    // Click background → deselect
    cyInstance.on('tap', (evt: any) => {
      if (evt.target === cyInstance) {
        selectedNode = null;
        selectedPath = null;
        branchMenu = null;
        cyInstance.nodes('.block').style({ opacity: 1 });
        cyInstance.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
      }
    });

    // Drag ANY node → move its children together (CFG blocks, seq descendants)
    cyInstance.on('drag', 'node', (evt: any) => {
      const node = evt.target;
      const nodeId = node.id();
      const nodeType = node.data('_type');
      const delta = { x: evt.position.x - node.data('_prevX'), y: evt.position.y - node.data('_prevY') };
      node.data('_prevX', evt.position.x);
      node.data('_prevY', evt.position.y);

      let children = cyInstance.collection();
      if (nodeType === 'function') {
        const funcName = node.data('label');
        if (expandedFuncs.has(funcName)) {
          children = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
        }
        // Also move seq descendants if this function has seq expansions
        const seqDesc = cyInstance.nodes().filter((n: any) => {
          const sp = n.data('_seqParent');
          return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
        });
        children = children.union(seqDesc);
      } else if (nodeType === 'seq-next') {
        children = cyInstance.nodes().filter((n: any) => {
          const sp = n.data('_seqParent');
          return sp && (sp === nodeId || sp.startsWith(nodeId + '→'));
        });
      } else if (nodeType === 'block') {
        // CFG blocks don't have children, nothing extra
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
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
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

    // Position the new node offset from existing children
    const existingChildren = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
    const isVertical = seqDirection === 'TB';
    if (existingChildren.length > 1) {
      const bb = existingChildren.not(cyInstance.getElementById(nodeId)).boundingBox();
      const targetPos = isVertical
        ? { x: bb.x2 + 140, y: bb.y1 + (bb.h / 2) }
        : { x: bb.x1 + (bb.w / 2), y: bb.y2 + 60 };
      cyInstance.getElementById(nodeId).animate({ position: targetPos }, { duration: 250, easing: 'ease-out' });
    } else {
      const targetPos = isVertical
        ? { x: parentPos.x, y: parentPos.y + 70 }
        : { x: parentPos.x + 160, y: parentPos.y };
      cyInstance.getElementById(nodeId).animate({ position: targetPos }, { duration: 250, easing: 'ease-out' });
    }
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    if (!cyInstance || !contract || !seqTree) return;
    const seqKey = parentNodeId;
    const parentNode = cyInstance.getElementById(parentNodeId);

    if (seqExpanded.has(seqKey)) {
      // Collapse: remove this node's auto-expanded descendants, but keep branches
      const directChildren = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
      const autoChildren = directChildren.filter((n: any) => !n.data('_isBranch'));
      // Collect all descendants of auto-children (recursively)
      const toRemoveNodes = cyInstance.collection();
      const toRemoveEdges = cyInstance.collection();
      function collectDesc(nodeId: string) {
        const children = cyInstance.nodes(`[_seqParent = "${nodeId}"]`);
        children.forEach((c: any) => {
          toRemoveNodes.merge(c);
          collectDesc(c.id());
        });
        const edges = cyInstance.edges().filter((e: any) => e.data('_seqParent') === nodeId);
        toRemoveEdges.merge(edges);
      }
      autoChildren.forEach((n: any) => {
        toRemoveNodes.merge(n);
        collectDesc(n.id());
      });
      // Edges from parent to auto-children
      const parentEdges = cyInstance.edges(`[_seqParent = "${seqKey}"]`).filter((e: any) => {
        const targetNode = cyInstance.getElementById(e.data('target'));
        return targetNode.length && !targetNode.data('_isBranch');
      });
      toRemoveEdges.merge(parentEdges);
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

  // Palette: dark board with blue tones (Excalidraw-like)
  const C = {
    bg: '#181a20',        // deep board background
    surface: '#1e2028',   // node backgrounds
    border: '#2a2d38',    // subtle borders
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
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.7,
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
  <div class="topbar">
    <a href="/">← Contracts</a>
    <span class="kind">{contract?.kind.toLowerCase() ?? ''}</span>
    <span class="cname">{contract?.name ?? 'Loading...'}</span>
    {#if contract?.inherits.length}
      <span class="inherits">inherits {contract.inherits.join(', ')}</span>
    {/if}
    <div class="toolbar">
      <button class="tbtn" class:active={mode === 'cfg'} onclick={() => switchMode('cfg')}>🔧 CFG</button>
      <button class="tbtn" class:active={mode === 'sequences'} onclick={() => switchMode('sequences')}>⚡ Sequences</button>
      {#if mode === 'sequences'}
        <span class="tsep"></span>
        <button class="tbtn" class:active={seqDirection === 'TB'} onclick={() => { seqDirection = 'TB'; }} title="Expand vertically">↓</button>
        <button class="tbtn" class:active={seqDirection === 'LR'} onclick={() => { seqDirection = 'LR'; }} title="Expand horizontally">→</button>
      {/if}
      <span class="tsep"></span>
      <button class="tbtn" onclick={() => runLayout(true)} title="Re-layout">⟳</button>
      <button class="tbtn" onclick={toggleSearch}>🔍</button>
      <button class="tbtn" onclick={() => { if (cyInstance) cyInstance.fit(undefined, 40); }}>⊡</button>
    </div>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {:else}
    <div class="canvas" bind:this={cyContainer}></div>

    {#if selectedNode && contract}
      <DraggablePanel
        title={selectedNode.label || ''}
        x={window.innerWidth - 370} y={60} width={350}
        onclose={() => selectedNode = null}
      >
        <div class="detail">
          {#if selectedNode._type === 'function'}
            <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.is_external ? 'External' : 'Internal'}</span></div>
            {#if !selectedNode.is_external}
              <div class="d-hint">Click on the node to {expandedFuncs.has(selectedNode.label) ? 'collapse' : 'expand'} its CFG</div>
            {/if}

            {#if funcPaths[selectedNode.label]}
              <div class="d-section">Paths ({funcPaths[selectedNode.label].stats.total_paths})</div>
              {#each funcPaths[selectedNode.label].paths as path}
                <button
                  class="d-path"
                  class:d-path-selected={selectedPath?.id === path.id}
                  onclick={() => highlightPath(selectedNode.label, path)}
                >
                  <span class="pid">#{path.id}</span>
                  <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
                  <span class="pdepth">{path.nodes.length}blk</span>
                  {#if path.annotations.external_calls.length > 0}
                    <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
                  {/if}
                  {#if path.annotations.state_writes.length > 0}
                    <span class="pb wr">✏{path.annotations.state_writes.length}</span>
                  {/if}
                </button>
              {/each}

              {#if selectedPath}
                <div class="path-detail-inline">
                  {#if selectedPath.annotations.require_checks.length > 0}
                    <div class="pd-title">Checks</div>
                    {#each selectedPath.annotations.require_checks as c}
                      <div class="pd-item check">{c}</div>
                    {/each}
                  {/if}
                  {#if selectedPath.annotations.external_calls.length > 0}
                    <div class="pd-title">External calls</div>
                    {#each selectedPath.annotations.external_calls as c}
                      <div class="pd-item ext">{c.target}.{c.function}()</div>
                    {/each}
                  {/if}
                  {#if selectedPath.annotations.state_writes.length > 0}
                    <div class="pd-title">State writes</div>
                    {#each selectedPath.annotations.state_writes as w}
                      <div class="pd-item wr">{w}</div>
                    {/each}
                  {/if}
                  {#if selectedPath.annotations.events_emitted.length > 0}
                    <div class="pd-title">Events</div>
                    {#each selectedPath.annotations.events_emitted as e}
                      <div class="pd-item ev">{e}</div>
                    {/each}
                  {/if}
                </div>
              {/if}
            {/if}
          {:else if selectedNode._type === 'seq-next'}
            <div class="d-row"><span class="d-label">Function</span><span>{selectedNode._funcName || selectedNode.label}</span></div>
            <div class="d-row"><span class="d-label">Paths</span><span>{selectedNode.pathCount}</span></div>
            <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.readOnly ? 'Read-only (view)' : 'State-changing'}</span></div>
            <div class="d-hint">Click → expand next steps · Double-click → expand CFG</div>

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
            <div class="d-row"><span class="d-label">Block</span><span>{selectedNode.node_type}</span></div>
            {#if selectedNode.statements?.length > 0}
              <div class="d-section">Statements</div>
              {#each selectedNode.statements as stmt}
                <div class="d-stmt">{stmt}</div>
              {/each}
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
  .view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #181a20; }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px; background: #1e2028; border-bottom: 1px solid #2a2d38;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #6b7a8d; text-decoration: none; }
  .topbar a:hover { color: #8bb8e8; }
  .kind { font-size: 12px; color: #6b7a8d; }
  .cname { font-size: 16px; font-weight: 700; color: #b8c4d4; }
  .inherits { font-size: 11px; color: #4a5568; font-style: italic; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; align-items: center; }
  .tbtn { background: #1e2028; border: 1px solid #2a2d38; color: #8bb8e8; padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; }
  .tbtn:hover { border-color: #5b9bd5; }
  .tbtn.active { background: #3a6b9f; border-color: #5b9bd5; color: #dce8f4; }
  .tsep { width: 1px; height: 18px; background: #2a2d38; }

  .error { padding: 24px; color: #b05050; }
  .canvas { flex: 1; }

  .detail { padding: 8px; }
  .d-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 12px; color: #b8c4d4; }
  .d-label { color: #6b7a8d; }
  .d-hint { font-size: 11px; color: #5b9bd5; padding: 6px 0; font-style: italic; }
  .d-section { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; font-weight: 600; }
  .d-chain-step { font-size: 11px; color: #8bb8e8; font-weight: 600; margin: 6px 0 2px; padding-top: 4px; border-top: 1px solid #2a2d38; }
  .d-path { display: flex; align-items: center; gap: 4px; padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit; background: transparent; border: 1px solid transparent; cursor: pointer; width: 100%; text-align: left; font: inherit; }
  .d-path:hover { background: #181a20; }
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
  .d-stmt { font-family: monospace; font-size: 11px; padding: 3px 6px; background: #181a20; border-radius: 3px; margin-bottom: 2px; color: #b8c4d4; }


  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px; font-size: 11px; color: #6b7a8d;
    background: #1e2028dd; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #2a2d38; z-index: 10;
    backdrop-filter: blur(8px);
  }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }

  .branch-menu {
    position: fixed; z-index: 60;
    background: #1e2028; border: 1px solid #2a2d38;
    border-radius: 8px; padding: 4px;
    box-shadow: 0 8px 32px #0a0b0f88;
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
  .branch-item:hover { background: #252830; color: #8bb8e8; }
  .branch-tag { font-size: 9px; color: #6b7a8d; background: #252830; padding: 1px 5px; border-radius: 8px; }
  .branch-close {
    display: block; width: 100%; padding: 5px 10px; margin-top: 2px;
    background: none; border: none; border-top: 1px solid #2a2d38;
    color: #4a5568; font-size: 11px; cursor: pointer; text-align: center;
  }
  .branch-close:hover { color: #b05050; }
</style>
