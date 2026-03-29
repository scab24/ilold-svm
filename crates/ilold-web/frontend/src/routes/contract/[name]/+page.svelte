<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { getContract, getCallGraph, getCfg, getPaths, getSequences, type ContractDetail, type CytoscapeGraph } from '$lib/api/rest';
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
  let seqExpanded: Map<string, boolean> = $state(new Map()); // "deposit" → expanded, "deposit→withdraw" → expanded
  let seqBreadcrumb: string[] = $state([]);

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

    // Click internal function → behavior depends on mode
    cyInstance.on('tap', 'node.internal', async (evt: any) => {
      const data = evt.target.data();
      if (data._type === 'function') {
        if (mode === 'cfg') {
          await toggleFuncExpand(data.label);
        } else if (mode === 'sequences') {
          await toggleSeqExpand(data.label, data.id);
        }
      } else if (data._type === 'seq-next') {
        // Click on a sequence next-step node → expand deeper
        await toggleSeqExpand(data.label, data.id);
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
        // Reset CFG block highlights
        cyInstance.nodes('.block').style({ opacity: 1 });
        cyInstance.edges('[_type = "cfg-edge"]').style({ opacity: 1 });
      }
    });

    // Drag function node → move its CFG children together
    cyInstance.on('drag', 'node[_type = "function"]', (evt: any) => {
      const node = evt.target;
      const funcName = node.data('label');
      if (!expandedFuncs.has(funcName)) return;

      const delta = { x: evt.position.x - node.data('_prevX'), y: evt.position.y - node.data('_prevY') };
      node.data('_prevX', evt.position.x);
      node.data('_prevY', evt.position.y);

      const children = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
      children.forEach((child: any) => {
        const pos = child.position();
        child.position({ x: pos.x + delta.x, y: pos.y + delta.y });
      });
    });

    cyInstance.on('grab', 'node[_type = "function"]', (evt: any) => {
      const pos = evt.target.position();
      evt.target.data('_prevX', pos.x);
      evt.target.data('_prevY', pos.y);
    });

    cyInstance.on('mouseover', 'node.internal', () => { if (cyContainer) cyContainer.style.cursor = 'pointer'; });
    cyInstance.on('mouseout', 'node', () => { if (cyContainer) cyContainer.style.cursor = 'default'; });
  }

  async function toggleFuncExpand(funcName: string) {
    if (!cyInstance || !contract) return;

    if (expandedFuncs.has(funcName)) {
      // COLLAPSE: animate children to parent, then remove
      const children = cyInstance.nodes(`[_parentFunc = "${funcName}"]`);
      const childEdges = cyInstance.edges(`[_parentFunc = "${funcName}"]`);
      const parentNode = cyInstance.getElementById(`${contract.name}::${funcName}`);
      const parentPos = parentNode.position();

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
      const parentId = `${contract.name}::${funcName}`;
      const parentPos = cyInstance.getElementById(parentId).position();

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

      // Dim all other function nodes and call edges so CFG stands out
      cyInstance.nodes('[_type = "function"]').style({ opacity: 0.15 });
      cyInstance.edges('[_type = "call"]').style({ opacity: 0.08 });
      // Keep the expanded function visible
      cyInstance.getElementById(parentId).style({ opacity: 1 });

      expandedFuncs.add(funcName);
      expandedFuncs = new Set(expandedFuncs);
    }
  }

  async function toggleSeqExpand(funcName: string, parentNodeId: string) {
    if (!cyInstance || !contract || !seqTree) return;
    const seqKey = parentNodeId;

    if (seqExpanded.has(seqKey)) {
      // Collapse: remove seq children
      const children = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
      const childEdges = cyInstance.edges(`[_seqParent = "${seqKey}"]`);
      // Also remove grandchildren recursively
      const allDesc = cyInstance.nodes().filter((n: any) => {
        const sp = n.data('_seqParent');
        return sp && (sp === seqKey || sp.startsWith(seqKey + '→'));
      });
      const allDescEdges = cyInstance.edges().filter((e: any) => {
        const sp = e.data('_seqParent');
        return sp && (sp === seqKey || sp.startsWith(seqKey + '→'));
      });
      cyInstance.remove(allDescEdges);
      cyInstance.remove(allDesc);
      seqExpanded.delete(seqKey);
      // Remove deeper keys too
      for (const k of seqExpanded.keys()) {
        if (k.startsWith(seqKey + '→')) seqExpanded.delete(k);
      }
      seqExpanded = new Map(seqExpanded);
    } else {
      // Expand: add next-step function nodes
      const parentPos = cyInstance.getElementById(parentNodeId).position();
      const funcs = seqTree.functions;
      const newNodes: any[] = [];
      const newEdges: any[] = [];

      funcs.forEach((f: any, i: number) => {
        const nodeId = `${seqKey}→${f.name}`;
        const readOnly = f.read_only;
        newNodes.push({
          group: 'nodes',
          data: {
            id: nodeId,
            label: f.name,
            _type: 'seq-next',
            _seqParent: seqKey,
            pathCount: f.path_count,
            readOnly,
          },
          position: { x: parentPos.x, y: parentPos.y },
          classes: `seq-next ${readOnly ? 'readonly' : 'state-change'}`,
        });
        newEdges.push({
          group: 'edges',
          data: {
            id: `se:${seqKey}→${f.name}`,
            source: parentNodeId,
            target: nodeId,
            _seqParent: seqKey,
          },
          classes: 'seq-edge',
        });
      });

      cyInstance.add([...newNodes, ...newEdges]);

      // Layout only the new nodes below parent
      const seqNodes = cyInstance.nodes(`[_seqParent = "${seqKey}"]`);
      const seqEdges = cyInstance.edges(`[_seqParent = "${seqKey}"]`);
      const seqElements = seqNodes.union(seqEdges);

      const subLayout = seqElements.layout({
        name: 'dagre',
        rankDir: 'LR',
        nodeSep: 15,
        rankSep: 50,
        animate: false,
        fit: false,
      } as any);
      subLayout.run();
      subLayout.stop();

      // Offset to be to the right of parent
      const bb = seqNodes.boundingBox();
      const offsetX = parentPos.x + 180 - bb.x1;
      const offsetY = parentPos.y - bb.y1 - bb.h / 2 + 15;

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

  function getStyles() {
    return [
      // Function nodes
      {
        selector: 'node.internal',
        style: {
          'background-color': '#238636', 'label': 'data(label)', 'color': '#f0f6fc',
          'font-size': '12px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '150px', 'height': '40px', 'shape': 'roundrectangle',
        }
      },
      {
        selector: 'node.external',
        style: {
          'background-color': '#161b22', 'label': 'data(label)', 'color': '#f85149',
          'font-size': '11px', 'text-valign': 'center', 'text-halign': 'center',
          'width': '130px', 'height': '34px', 'shape': 'roundrectangle',
          'border-style': 'dashed', 'border-width': 1, 'border-color': '#f85149',
        }
      },
      // CFG block nodes
      {
        selector: 'node.block',
        style: {
          'label': 'data(label)', 'color': '#c9d1d9', 'font-size': '9px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '160px', 'height': '30px', 'shape': 'roundrectangle',
          'background-color': '#21262d', 'border-width': 1, 'border-color': '#30363d',
          'text-max-width': '150px', 'text-wrap': 'ellipsis',
        }
      },
      { selector: 'node.block-entry', style: { 'background-color': '#1f6feb', 'border-color': '#58a6ff', 'color': '#f0f6fc' } },
      { selector: 'node.block-return', style: { 'background-color': '#238636', 'border-color': '#3fb950', 'color': '#f0f6fc', 'width': '90px' } },
      { selector: 'node.block-revert', style: { 'background-color': '#da3633', 'border-color': '#f85149', 'color': '#f0f6fc', 'width': '90px' } },
      { selector: 'node.block-loopcondition', style: { 'background-color': '#9e6a03', 'border-color': '#d29922', 'color': '#f0f6fc', 'shape': 'diamond', 'width': '90px', 'height': '45px' } },
      { selector: 'node:active', style: { 'overlay-opacity': 0 } },
      // Call edges
      {
        selector: 'edge[_type = "call"]',
        style: {
          'width': 1.5, 'line-color': '#484f58', 'target-arrow-color': '#484f58',
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.8,
        }
      },
      {
        selector: 'edge[kind = "External"]',
        style: { 'line-color': '#f8514966', 'target-arrow-color': '#f85149', 'line-style': 'dashed' }
      },
      // CFG edges
      {
        selector: 'edge[_type = "cfg-edge"]',
        style: {
          'width': 1, 'line-color': '#30363d', 'target-arrow-color': '#30363d',
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.6,
        }
      },
      { selector: 'edge.cond-true', style: { 'line-color': '#3fb95088', 'target-arrow-color': '#3fb950', 'label': '✓', 'font-size': '11px', 'color': '#3fb950' } },
      { selector: 'edge.cond-false', style: { 'line-color': '#f8514988', 'target-arrow-color': '#f85149', 'label': '✗', 'font-size': '11px', 'color': '#f85149' } },
      { selector: 'edge.loop-back', style: { 'line-color': '#d29922', 'target-arrow-color': '#d29922', 'line-style': 'dashed' } },
      { selector: 'edge.expand-link', style: { 'line-color': '#58a6ff44', 'target-arrow-color': '#58a6ff', 'line-style': 'dotted', 'width': 2 } },
      // Sequence nodes
      {
        selector: 'node.seq-next',
        style: {
          'label': 'data(label)', 'color': '#c9d1d9', 'font-size': '10px',
          'text-valign': 'center', 'text-halign': 'center',
          'width': '110px', 'height': '28px', 'shape': 'roundrectangle',
          'background-color': '#238636', 'border-width': 1, 'border-color': '#3fb950',
        }
      },
      { selector: 'node.seq-next.readonly', style: { 'background-color': '#1f6feb', 'border-color': '#58a6ff' } },
      { selector: 'node.seq-next.state-change', style: { 'background-color': '#238636', 'border-color': '#3fb950' } },
      {
        selector: 'edge.seq-edge',
        style: {
          'width': 1.5, 'line-color': '#d2992244', 'target-arrow-color': '#d29922',
          'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.7,
        }
      },
    ];
  }

  function termColor(t: string): string {
    return t === 'Return' ? '#3fb950' : t === 'Revert' ? '#f85149' : '#8b949e';
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

    <div class="legend">
      {#if mode === 'cfg'}
        <span><span class="dot" style="background:#238636"></span>Function</span>
        <span><span class="dot" style="background:#1f6feb"></span>Entry</span>
        <span><span class="dot" style="background:#da3633"></span>Revert</span>
        <span>Click function → expand CFG</span>
      {:else}
        <span><span class="dot" style="background:#238636"></span>State-changing</span>
        <span><span class="dot" style="background:#1f6feb"></span>Read-only</span>
        <span>Click function → show next-step combinations</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .view { position: fixed; inset: 0; display: flex; flex-direction: column; background: #0d1117; }

  .topbar {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 16px; background: #161b22; border-bottom: 1px solid #30363d;
    z-index: 10; flex-shrink: 0;
  }
  .topbar a { font-size: 13px; color: #8b949e; }
  .kind { font-size: 12px; color: #8b949e; }
  .cname { font-size: 16px; font-weight: 700; color: #f0f6fc; }
  .inherits { font-size: 11px; color: #484f58; font-style: italic; }
  .toolbar { margin-left: auto; display: flex; gap: 4px; }
  .tbtn { background: #21262d; border: 1px solid #30363d; color: #c9d1d9; padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; }
  .tbtn:hover { border-color: #58a6ff; }
  .tbtn.active { background: #1f6feb; border-color: #58a6ff; color: #f0f6fc; }

  .error { padding: 24px; color: #f85149; }
  .canvas { flex: 1; }

  .detail { padding: 8px; }
  .d-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 12px; }
  .d-label { color: #8b949e; }
  .d-hint { font-size: 11px; color: #58a6ff; padding: 6px 0; font-style: italic; }
  .d-section { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; font-weight: 600; }
  .d-path { display: flex; align-items: center; gap: 4px; padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit; background: transparent; border: 1px solid transparent; cursor: pointer; width: 100%; text-align: left; font: inherit; }
  .d-path:hover { background: #0d1117; }
  .pid { color: #484f58; font-weight: 600; }
  .pdepth { color: #484f58; font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: #f851491a; color: #f85149; }
  .pb.wr { background: #58a6ff1a; color: #58a6ff; }
  .d-path-selected { background: #21262d; border-color: #58a6ff; }

  .path-detail-inline {
    margin-top: 8px; padding-top: 8px;
    border-top: 1px solid #21262d;
  }
  .pd-title { font-size: 9px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin: 6px 0 2px; }
  .pd-item {
    font-family: monospace; font-size: 11px;
    padding: 2px 6px; border-radius: 3px; margin-bottom: 2px;
  }
  .pd-item.check { background: #d299221a; color: #d29922; }
  .pd-item.ext { background: #f851491a; color: #f85149; }
  .pd-item.wr { background: #58a6ff1a; color: #58a6ff; }
  .pd-item.ev { background: #3fb9501a; color: #3fb950; }
  .d-stmt { font-family: monospace; font-size: 11px; padding: 3px 6px; background: #0d1117; border-radius: 3px; margin-bottom: 2px; color: #c9d1d9; }

  .legend {
    position: fixed; bottom: 12px; left: 16px;
    display: flex; gap: 10px; font-size: 11px; color: #8b949e;
    background: #161b22cc; padding: 6px 12px;
    border-radius: 6px; border: 1px solid #30363d; z-index: 10;
  }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 2px; vertical-align: middle; margin-right: 3px; }
</style>
