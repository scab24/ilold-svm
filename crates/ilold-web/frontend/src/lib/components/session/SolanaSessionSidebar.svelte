<script lang="ts">
  import EmbeddedTerminal from './EmbeddedTerminal.svelte';
  import NodeInspector from '$lib/components/contract/NodeInspector.svelte';
  import type { ProgramDetail } from '$lib/api/rest';
  import type { TraceNodeData } from '$lib/stores/graph.svelte';
  import { getNodes } from '$lib/stores/graph.svelte';

  let {
    program,
    selectedNode,
    users,
    onsolanarun,
    onnewuser,
    onairdrop,
    onstepsync,
  }: {
    program: ProgramDetail;
    selectedNode: any;
    users: { name: string; pubkey: string; lamports: number }[];
    onsolanarun: (ix: string) => void;
    onnewuser: (name: string, lamports: number) => Promise<void>;
    onairdrop: (name: string, lamports: number) => Promise<void>;
    onstepsync?: () => Promise<void>;
  } = $props();

  let activeTab: 'steps' | 'users' | 'inspector' = $state('steps');
  let newUserName = $state('');
  let newUserLamports = $state(10_000_000_000);
  let pendingUser = $state(false);
  let userError = $state<string | null>(null);

  let prevSelectedId = $state<string | null>(null);
  $effect(() => {
    const id = selectedNode?.id ?? null;
    if (id && id !== prevSelectedId) {
      activeTab = 'inspector';
    }
    prevSelectedId = id;
  });

  const traceSteps = $derived.by(() => {
    return getNodes()
      .filter((n) => (n.data as any)?._type === 'trace')
      .map((n) => n.data as TraceNodeData)
      .sort((a, b) => a.stepIndex - b.stepIndex);
  });

  async function handleCreateUser(e: SubmitEvent) {
    e.preventDefault();
    userError = null;
    if (!newUserName.trim()) {
      userError = 'name required';
      return;
    }
    pendingUser = true;
    try {
      await onnewuser(newUserName.trim(), newUserLamports);
      newUserName = '';
    } catch (e) {
      userError = e instanceof Error ? e.message : String(e);
    } finally {
      pendingUser = false;
    }
  }

  async function topUp(name: string) {
    try {
      await onairdrop(name, 1_000_000_000);
    } catch (e) {
      alert(`airdrop failed:\n\n${e instanceof Error ? e.message : String(e)}`);
    }
  }
</script>

<div class="solana-sidebar flex flex-col h-full bg-hover border-l border-border-subtle">
  <div class="flex px-1.5 mb-0 tab-header">
    <button
      class="tab-btn"
      class:active={activeTab === 'steps'}
      onclick={() => (activeTab = 'steps')}
    >Steps</button>
    <button
      class="tab-btn"
      class:active={activeTab === 'users'}
      onclick={() => (activeTab = 'users')}
    >Users</button>
    <button
      class="tab-btn"
      class:active={activeTab === 'inspector'}
      onclick={() => (activeTab = 'inspector')}
      title={selectedNode ? `Inspect: ${selectedNode.label}` : 'Inspector'}
    >
      Inspector{#if selectedNode}<span class="ml-1 text-accent">●</span>{/if}
    </button>
  </div>

  <div class="flex-1 overflow-y-auto min-h-0 px-2 py-2">
    {#if activeTab === 'steps'}
      {#if traceSteps.length === 0}
        <div class="empty">
          <div class="empty-title">No steps yet</div>
          <div class="empty-hint">Click an instruction in the sidebar and press Execute, or run <code>call &lt;ix&gt; &lt;json&gt;</code> in the terminal.</div>
        </div>
      {/if}
      {#each traceSteps as step (step.stepIndex)}
        <div class="step-row">
          <div class="step-head">
            <span class="step-index">#{step.stepIndex}</span>
            <span class="step-name">{step.instruction}</span>
            {#if step.error}
              <span class="step-error">err</span>
            {/if}
          </div>
          <div class="step-meta">
            <span>{step.computeUnits} CU</span>
            <span>{step.diffsCount} diffs</span>
            <span class="step-scn">{step.scenario}</span>
          </div>
          {#if step.logsExcerpt && step.logsExcerpt.length > 0}
            <pre class="step-logs">{step.logsExcerpt.slice(0, 5).join('\n')}</pre>
          {/if}
        </div>
      {/each}
    {:else if activeTab === 'users'}
      <form class="user-form" onsubmit={handleCreateUser}>
        <div class="user-form-row">
          <input
            class="user-input"
            type="text"
            bind:value={newUserName}
            placeholder="user name (e.g. admin)"
            disabled={pendingUser}
          />
          <input
            class="user-input lamports"
            type="number"
            bind:value={newUserLamports}
            placeholder="lamports"
            disabled={pendingUser}
          />
          <button class="user-add" type="submit" disabled={pendingUser}>
            {pendingUser ? '…' : '+'}
          </button>
        </div>
        {#if userError}
          <div class="user-error">{userError}</div>
        {/if}
      </form>

      {#if users.length === 0}
        <div class="empty">
          <div class="empty-title">No users yet</div>
          <div class="empty-hint">Create one above or run <code>users new &lt;name&gt;</code> in the terminal. Names autocomplete in the run panel.</div>
        </div>
      {/if}
      {#each users as u (u.name)}
        <div class="user-row">
          <div class="user-head">
            <span class="user-name">{u.name}</span>
            <span class="user-balance">{u.lamports.toLocaleString()} lamports</span>
          </div>
          <div class="user-pk">{u.pubkey}</div>
          <button class="user-air" onclick={() => topUp(u.name)} title="Airdrop +1 SOL">+1 SOL</button>
        </div>
      {/each}
    {:else}
      <NodeInspector
        {selectedNode}
        selectedPath={null}
        funcPaths={{}}
        expandedFuncs={new Set()}
        seqExpanded={new Map()}
        mode="cfg"
        seqAnalysis={null}
        contract={{ name: program.name, functions: [] }}
        lookupBlock={() => null}
        onpathselect={() => {}}
        onexpandcfg={() => {}}
        {onsolanarun}
      />
    {/if}
  </div>

  <EmbeddedTerminal />
</div>

<style>
  .solana-sidebar {
    width: 360px;
  }
  .tab-header {
    border-bottom: 1px solid var(--color-border-subtle);
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
  }
  .tab-btn {
    flex: 1;
    padding: 8px 0;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .tab-btn.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
  }
  .empty {
    text-align: center;
    padding: 28px 12px;
    color: var(--color-text-dim);
  }
  .empty-title {
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .empty-hint {
    margin-top: 6px;
    font-size: 11px;
    line-height: 1.5;
  }
  .empty-hint code {
    background: var(--color-dark);
    padding: 1px 5px;
    border-radius: 3px;
  }
  .step-row {
    background: var(--color-dark);
    border: 1px solid var(--color-border-subtle);
    border-radius: 6px;
    padding: 8px;
    margin-bottom: 8px;
  }
  .step-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .step-index {
    font-size: 10px;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
  }
  .step-name {
    font-family: var(--font-mono, monospace);
    font-weight: 700;
    color: var(--color-text);
  }
  .step-error {
    font-size: 9px;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(220, 80, 80, 0.15);
    color: var(--color-danger);
  }
  .step-meta {
    display: flex;
    gap: 10px;
    font-size: 10px;
    color: var(--color-text-dim);
    margin-top: 4px;
  }
  .step-scn {
    margin-left: auto;
    color: var(--color-accent-hover);
  }
  .step-logs {
    margin-top: 6px;
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-hover);
    border-radius: 4px;
    padding: 6px;
    max-height: 80px;
    overflow-y: auto;
    white-space: pre-wrap;
  }
  .user-form {
    margin-bottom: 12px;
  }
  .user-form-row {
    display: flex;
    gap: 4px;
  }
  .user-input {
    flex: 1;
    background: var(--color-dark);
    border: 1px solid var(--color-border-subtle);
    border-radius: 4px;
    padding: 5px 8px;
    font-size: 11px;
    color: var(--color-text);
    font-family: var(--font-mono, monospace);
  }
  .user-input.lamports {
    flex: 0 0 110px;
  }
  .user-add {
    width: 28px;
    background: var(--color-accent);
    border: none;
    border-radius: 4px;
    color: var(--color-dark);
    cursor: pointer;
    font-weight: 700;
  }
  .user-add:disabled,
  .user-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .user-error {
    margin-top: 6px;
    color: var(--color-danger);
    font-size: 11px;
  }
  .user-row {
    background: var(--color-dark);
    border: 1px solid var(--color-border-subtle);
    border-radius: 6px;
    padding: 8px;
    margin-bottom: 6px;
    position: relative;
  }
  .user-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .user-name {
    font-weight: 700;
    font-family: var(--font-mono, monospace);
    color: var(--color-text);
  }
  .user-balance {
    margin-left: auto;
    font-size: 10px;
    color: var(--color-text-muted);
  }
  .user-pk {
    margin-top: 4px;
    font-size: 10px;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
    word-break: break-all;
  }
  .user-air {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background: var(--color-hover);
    border: 1px solid var(--color-border-subtle);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 10px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .user-air:hover {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }
</style>
