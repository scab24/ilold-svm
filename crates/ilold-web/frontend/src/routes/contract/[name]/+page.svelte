<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { getContract, getCallGraph, type ContractDetail, type CytoscapeGraph } from '$lib/api/rest';

  let contract: ContractDetail | null = $state(null);
  let callgraph: CytoscapeGraph | null = $state(null);
  let error: string | null = $state(null);

  let cyContainer: HTMLDivElement;

  const name = $derived(page.params.name);

  $effect(() => {
    if (name) loadContract(name);
  });

  async function loadContract(contractName: string) {
    try {
      contract = await getContract(contractName);
      callgraph = await getCallGraph(contractName);
      if (cyContainer && callgraph) renderCallGraph(callgraph);
    } catch (e) {
      error = `Contract "${contractName}" not found`;
    }
  }

  async function renderCallGraph(graph: CytoscapeGraph) {
    const cytoscape = (await import('cytoscape')).default;
    const dagre = (await import('cytoscape-dagre')).default;
    cytoscape.use(dagre);

    const cy = cytoscape({
      container: cyContainer,
      elements: [
        ...graph.nodes.map(n => ({ group: 'nodes' as const, data: n.data })),
        ...graph.edges.map(e => ({ group: 'edges' as const, data: e.data })),
      ],
      style: [
        {
          selector: 'node[node_type = "internal"]',
          style: {
            'background-color': '#238636',
            'label': 'data(label)',
            'color': '#c9d1d9',
            'font-size': '12px',
            'text-valign': 'center',
            'text-halign': 'center',
            'width': '120px',
            'height': '40px',
            'shape': 'roundrectangle',
            'text-wrap': 'wrap',
          }
        },
        {
          selector: 'node[node_type = "external"]',
          style: {
            'background-color': '#da3633',
            'label': 'data(label)',
            'color': '#f0f6fc',
            'font-size': '11px',
            'text-valign': 'center',
            'text-halign': 'center',
            'width': '110px',
            'height': '35px',
            'shape': 'roundrectangle',
            'border-style': 'dashed',
            'border-width': 2,
            'border-color': '#f85149',
          }
        },
        {
          selector: 'edge',
          style: {
            'width': 2,
            'line-color': '#30363d',
            'target-arrow-color': '#30363d',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'font-size': '10px',
            'color': '#8b949e',
          }
        },
        {
          selector: 'edge[kind = "External"]',
          style: {
            'line-color': '#da3633',
            'target-arrow-color': '#da3633',
            'line-style': 'dashed',
          }
        },
      ],
      layout: {
        name: 'dagre',
        rankDir: 'TB',
        nodeSep: 60,
        rankSep: 80,
      },
    });

    cy.on('tap', 'node', (evt: any) => {
      const nodeData = evt.target.data();
      if (!nodeData.is_external) {
        const funcName = nodeData.label;
        if (funcName && contract) {
          window.location.href = `/contract/${contract.name}/${funcName}`;
        }
      }
    });
  }
</script>

<div class="contract-detail">
  {#if error}
    <div class="error">{error}</div>
  {:else if !contract}
    <div class="loading">Loading...</div>
  {:else}
    <div class="header">
      <a href="/">← Contracts</a>
      <h1>
        <span class="kind">{contract.kind.toLowerCase()}</span>
        {contract.name}
      </h1>
      {#if contract.inherits.length > 0}
        <p class="inherits">inherits {contract.inherits.join(', ')}</p>
      {/if}
    </div>

    <div class="content">
      <div class="callgraph-section">
        <h2>Call Graph</h2>
        <div class="cy-container" bind:this={cyContainer}></div>
      </div>

      <div class="functions-section">
        <h2>Functions ({contract.functions.length})</h2>
        <div class="func-list">
          {#each contract.functions as func}
            <a href="/contract/{contract.name}/{func.name || 'constructor'}" class="func-row">
              <div class="func-info">
                <span class="vis">{func.visibility.toLowerCase()}</span>
                <span class="func-name">{func.name || 'constructor'}</span>
                {#if func.params.length > 0}
                  <span class="params">({func.params.map(p => p.type_name).join(', ')})</span>
                {/if}
              </div>
              <div class="func-stats">
                <span class="paths">{func.path_count} paths</span>
                {#if func.happy_paths > 0}
                  <span class="happy">{func.happy_paths} ✓</span>
                {/if}
                {#if func.revert_paths > 0}
                  <span class="revert">{func.revert_paths} ✗</span>
                {/if}
              </div>
            </a>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .contract-detail {
    max-width: 1200px;
  }

  .error {
    color: #f85149;
    padding: 16px;
    border: 1px solid #f8514933;
    border-radius: 8px;
    background: #f851491a;
  }

  .loading {
    color: #8b949e;
  }

  .header {
    margin-bottom: 24px;
  }

  .header a {
    font-size: 13px;
    color: #8b949e;
  }

  .header h1 {
    font-size: 28px;
    margin: 8px 0 0 0;
  }

  .kind {
    font-size: 14px;
    color: #8b949e;
    margin-right: 8px;
  }

  .inherits {
    color: #8b949e;
    font-size: 13px;
    margin: 4px 0 0 0;
  }

  .content {
    display: grid;
    grid-template-columns: 1fr 400px;
    gap: 24px;
  }

  .callgraph-section h2,
  .functions-section h2 {
    font-size: 16px;
    margin: 0 0 12px 0;
    color: #f0f6fc;
  }

  .cy-container {
    width: 100%;
    height: 500px;
    background: #0d1117;
    border: 1px solid #30363d;
    border-radius: 8px;
  }

  .func-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .func-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 6px;
    color: inherit;
    transition: border-color 0.15s;
  }

  .func-row:hover {
    border-color: #58a6ff;
    text-decoration: none;
  }

  .func-info {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }

  .vis {
    font-size: 11px;
    color: #8b949e;
    min-width: 55px;
  }

  .func-name {
    font-weight: 600;
    color: #f0f6fc;
  }

  .params {
    font-size: 12px;
    color: #8b949e;
  }

  .func-stats {
    display: flex;
    gap: 8px;
    font-size: 12px;
  }

  .paths {
    color: #8b949e;
  }

  .happy {
    color: #3fb950;
  }

  .revert {
    color: #f85149;
  }
</style>
