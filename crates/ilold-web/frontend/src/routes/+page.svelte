<script lang="ts">
  import { onMount } from 'svelte';
  import { getProject, type ProjectSummary } from '$lib/api/rest';

  let project: ProjectSummary | null = $state(null);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      project = await getProject();
    } catch (e) {
      error = 'Failed to connect to server. Is "ilold serve" running?';
    }
  });
</script>

<div class="overview">
  <h1>Contracts</h1>

  {#if error}
    <div class="error">{error}</div>
  {:else if !project}
    <div class="loading">Analyzing...</div>
  {:else}
    <p class="summary">{project.files} file(s), {project.contracts.length} contract(s)</p>

    <div class="grid">
      {#each project.contracts as contract}
        <a href="/contract/{contract.name}" class="card">
          <div class="card-header">
            <span class="kind">{contract.kind.toLowerCase()}</span>
            <h2>{contract.name}</h2>
          </div>
          <div class="card-body">
            <div class="stat">
              <span class="number">{contract.functions}</span>
              <span class="label">functions</span>
            </div>
            <div class="stat">
              <span class="number">{contract.state_vars}</span>
              <span class="label">state vars</span>
            </div>
          </div>
          {#if contract.inherits.length > 0}
            <div class="inherits">inherits {contract.inherits.join(', ')}</div>
          {/if}
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .overview h1 {
    font-size: 24px;
    margin: 0 0 8px 0;
  }

  .summary {
    color: #8b949e;
    margin: 0 0 24px 0;
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

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }

  .card {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 16px;
    display: block;
    color: inherit;
    transition: border-color 0.15s;
  }

  .card:hover {
    border-color: #58a6ff;
    text-decoration: none;
  }

  .card-header {
    margin-bottom: 12px;
  }

  .kind {
    font-size: 11px;
    color: #8b949e;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .card-header h2 {
    font-size: 18px;
    margin: 4px 0 0 0;
    color: #f0f6fc;
  }

  .card-body {
    display: flex;
    gap: 24px;
  }

  .stat {
    display: flex;
    flex-direction: column;
  }

  .number {
    font-size: 20px;
    font-weight: 600;
    color: #58a6ff;
  }

  .label {
    font-size: 12px;
    color: #8b949e;
  }

  .inherits {
    margin-top: 12px;
    font-size: 12px;
    color: #8b949e;
    font-style: italic;
  }
</style>
