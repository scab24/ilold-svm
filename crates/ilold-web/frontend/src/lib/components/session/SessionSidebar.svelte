<script lang="ts">
  import { untrack } from 'svelte';
  import EmbeddedTerminal from './EmbeddedTerminal.svelte';
  import SessionTimeline from './SessionTimeline.svelte';
  import StatePanel from './StatePanel.svelte';
  import NodeInspector from '$lib/components/contract/NodeInspector.svelte';
  import { getScenarios, getActiveScenario } from '$lib/stores/session.svelte';
  import { promptScenarioName } from '$lib/scenarios/name';
  import { dispatchScenarioAction } from '$lib/scenarios/dispatch';
  import { getNodes } from '$lib/stores/graph.svelte';
  import type { ProgramDetail } from '$lib/api/rest';
  import type { TraceNodeData } from '$lib/stores/graph.svelte';

  let {
    contract,
    selectedNode = null,
    selectedPath = null,
    funcPaths = {},
    expandedFuncs = new Set<string>(),
    seqExpanded = new Map<string, boolean>(),
    mode = 'sequences',
    seqAnalysis = null,
    contractDetail = null,
    lookupBlock = () => null,
    onpathselect = () => {},
    onexpandcfg = () => {},
    kind = 'solidity',
    program = null,
    solanaUsers = [],
    onsolanarun = () => {},
    onnewuser = async () => {},
    onairdrop = async () => {},
  }: {
    contract: string;
    selectedNode?: any;
    selectedPath?: any;
    funcPaths?: Record<string, any>;
    expandedFuncs?: Set<string>;
    seqExpanded?: Map<string, boolean>;
    mode?: 'cfg' | 'sequences' | 'session' | 'program' | 'trace';
    seqAnalysis?: any;
    contractDetail?: { name: string; functions?: any[] } | null;
    lookupBlock?: (blockId: string) => { statements: string[]; node_type: string } | null;
    onpathselect?: (funcName: string, path: any) => void;
    onexpandcfg?: (funcName: string, nodeId?: string) => void;
    kind?: 'solidity' | 'solana';
    program?: ProgramDetail | null;
    solanaUsers?: { name: string; pubkey: string; lamports: number }[];
    onsolanarun?: (instruction: string) => void;
    onnewuser?: (name: string, lamports: number) => Promise<void>;
    onairdrop?: (name: string, lamports: number) => Promise<void>;
  } = $props();

  let newUserName = $state('');
  let newUserLamports = $state(10_000_000_000);
  let pendingUser = $state(false);
  let userError = $state<string | null>(null);

  const traceSteps = $derived.by<TraceNodeData[]>(() => {
    if (kind !== 'solana') return [];
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

  let open = $state(true);
  let activeTab: 'timeline' | 'state' | 'inspector' = $state('timeline');

  // Auto-switch to the Inspector tab the moment the user selects a node
  // on the canvas. Mirrors Figma / VSCode — selection should reveal the
  // details without the user having to hunt for a tab. We only switch on
  // the null → non-null transition so re-selecting a different node
  // doesn't keep yanking the user back to this tab mid-navigation.
  let prevSelectedId = $state<string | null>(null);
  $effect(() => {
    const currentId = selectedNode?.id ?? null;
    // Read the tracker via untrack so writing it at the end doesn't
    // re-trigger this effect — only selectedNode changes should flow in.
    const prev = untrack(() => prevSelectedId);
    if (currentId && !prev) {
      activeTab = 'inspector';
    }
    prevSelectedId = currentId;
  });

  // Reactive view of scenarios (ordered map + active name, both from session store)
  const scenarioEntries = $derived(Array.from(getScenarios().entries()));
  const activeScenario = $derived(getActiveScenario());

  async function switchScenario(name: string) {
    if (name === activeScenario) return;
    await dispatchScenarioAction({ Switch: { name } }, contract, 'switch');
  }

  async function newScenario() {
    const name = promptScenarioName();
    if (!name) return;
    await dispatchScenarioAction({ New: { name } }, contract, 'new');
  }

  // Backend guards: cannot delete the active scenario or the only one.
  // The pill hides ✕ on the active row, so we only reach here for safe cases.
  async function deleteScenario(name: string) {
    await dispatchScenarioAction({ Delete: { name } }, contract, 'delete');
  }

  // Resizable sidebar width (drag handle on left edge)
  let sidebarWidth = $state(480);
  const MIN_WIDTH = 320;
  const MAX_WIDTH = 900;
  let draggingWidth = $state(false);

  function onWidthDragStart(e: MouseEvent) {
    e.preventDefault();
    draggingWidth = true;
    document.body.style.userSelect = 'none';
    const startX = e.clientX;
    const startW = sidebarWidth;

    function onMove(ev: MouseEvent) {
      // Sidebar grows to the left, so drag left = wider
      const delta = startX - ev.clientX;
      sidebarWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW + delta));
    }
    function onUp() {
      draggingWidth = false;
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }
</script>

<div
  class="flex flex-col flex-shrink-0 relative h-full"
  style="
    width: {open ? `${sidebarWidth}px` : '28px'};
    background: linear-gradient(180deg, rgba(20, 20, 28, 0.95) 0%, rgba(16, 16, 22, 0.98) 100%);
    border-left: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
  "
>
  <!-- Sidebar width drag handle (left edge) -->
  {#if open}
    <div
      class="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize z-10 {draggingWidth ? 'bg-accent-dark' : 'hover:bg-surface-alt'}"
      onmousedown={onWidthDragStart}
      role="separator"
      aria-orientation="vertical"
    ></div>
  {/if}

  <button
    class="absolute left-1 top-2 border cursor-pointer px-[3px] py-1 text-[10px] z-5 text-text-muted transition-colors duration-150 hover:text-accent-hover"
    style="
      border-radius: 6px;
      border-color: color-mix(in srgb, var(--color-border) 40%, transparent);
      background: rgba(30, 30, 40, 0.8);
      backdrop-filter: blur(8px);
    "
    onclick={() => open = !open}
  >
    {open ? '▸' : '◂'}
  </button>

  <div class="flex flex-col flex-1 min-h-0" class:hidden={!open}>
    <!-- Scenario selector — pill tabs, one per scenario plus a + button -->
    <div
      class="flex items-center gap-1 px-2 py-1.5 overflow-x-auto shrink-0"
      style="
        background: color-mix(in srgb, var(--color-surface) 60%, transparent);
        border-bottom: 1px solid color-mix(in srgb, var(--color-border) 30%, transparent);
      "
    >
      <span class="text-[9px] uppercase tracking-wider text-text-dim font-semibold mr-1 shrink-0">Scenarios</span>
      {#each scenarioEntries as [name, steps] (name)}
        {@const isActive = name === activeScenario}
        <div
          class="shrink-0 inline-flex items-stretch border transition-colors duration-150"
          style="
            border-radius: 999px;
            font-family: var(--font-mono), monospace;
            border-color: {isActive ? 'var(--color-accent)' : 'color-mix(in srgb, var(--color-border) 50%, transparent)'};
            background: {isActive ? 'color-mix(in srgb, var(--color-accent) 18%, transparent)' : 'rgba(30, 30, 40, 0.6)'};
          "
        >
          <button
            class="bg-transparent border-none {isActive ? 'text-accent-light' : 'text-text-muted hover:text-text'} transition-colors duration-150 cursor-pointer"
            style="padding: 3px {isActive ? '8px' : '4px'} 3px 8px; font-size: 10px; font-family: inherit; border-radius: {isActive ? '999px' : '999px 0 0 999px'};"
            onclick={() => switchScenario(name)}
            title={isActive ? `Active scenario: ${name}` : `Switch to ${name}`}
          >
            {name} <span class="text-text-dim ml-0.5">• {steps.length}</span>
          </button>
          {#if !isActive}
            <button
              class="bg-transparent border-none text-text-dim hover:text-danger transition-colors duration-150 cursor-pointer"
              style="padding: 3px 6px 3px 2px; font-size: 9px; font-family: inherit; border-radius: 0 999px 999px 0;"
              onclick={() => deleteScenario(name)}
              title={`Delete scenario ${name}`}
              aria-label={`Delete scenario ${name}`}
            >
              ✕
            </button>
          {/if}
        </div>
      {/each}
      <button
        class="shrink-0 border cursor-pointer text-text-muted hover:text-accent-hover transition-colors duration-150"
        style="
          padding: 3px 8px;
          border-radius: 999px;
          font-size: 11px;
          font-weight: 600;
          border-color: color-mix(in srgb, var(--color-border) 50%, transparent);
          background: rgba(30, 30, 40, 0.6);
        "
        onclick={newScenario}
        title="Create new scenario"
      >
        +
      </button>
    </div>

    <!-- Tab header — glass effect -->
    <div
      class="flex px-1.5 mb-0"
      style="
        border-bottom: 1px solid color-mix(in srgb, var(--color-border) 40%, transparent);
        background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
        backdrop-filter: blur(16px) saturate(1.8);
        -webkit-backdrop-filter: blur(16px) saturate(1.8);
      "
    >
      <button
        class="flex-1 py-2 bg-transparent border-none text-[10px] font-semibold uppercase tracking-wider cursor-pointer transition-colors duration-150 {activeTab === 'timeline' ? 'text-accent' : 'text-text-muted hover:text-text'}"
        style="border-bottom: 2px solid {activeTab === 'timeline' ? 'var(--color-accent)' : 'transparent'};"
        onclick={() => activeTab = 'timeline'}
      >
        Timeline
      </button>
      <button
        class="flex-1 py-2 bg-transparent border-none text-[10px] font-semibold uppercase tracking-wider cursor-pointer transition-colors duration-150 {activeTab === 'state' ? 'text-accent' : 'text-text-muted hover:text-text'}"
        style="border-bottom: 2px solid {activeTab === 'state' ? 'var(--color-accent)' : 'transparent'};"
        onclick={() => activeTab = 'state'}
      >
        State
      </button>
      <button
        class="flex-1 py-2 bg-transparent border-none text-[10px] font-semibold uppercase tracking-wider cursor-pointer transition-colors duration-150 {activeTab === 'inspector' ? 'text-accent' : 'text-text-muted hover:text-text'}"
        style="border-bottom: 2px solid {activeTab === 'inspector' ? 'var(--color-accent)' : 'transparent'};"
        onclick={() => activeTab = 'inspector'}
        title={selectedNode ? `Inspect: ${selectedNode._funcName || selectedNode.label}` : 'Inspector — select a node to populate'}
      >
        Inspector{#if selectedNode}<span class="ml-1 text-accent-light">●</span>{/if}
      </button>
    </div>

    <div class="flex-1 overflow-y-auto min-h-0 px-1">
      {#if activeTab === 'timeline'}
        {#if kind === 'solana'}
          {#if traceSteps.length === 0}
            <div class="solana-empty">
              <div class="solana-empty-title">No steps yet</div>
              <div class="solana-empty-hint">Click an instruction in the sidebar and press Execute, or run <code>call &lt;ix&gt; &lt;json&gt;</code> in the terminal.</div>
            </div>
          {/if}
          {#each traceSteps as step (step.stepIndex)}
            <div class="trace-step">
              <div class="trace-step-head">
                <span class="trace-step-idx">#{step.stepIndex}</span>
                <span class="trace-step-name">{step.instruction}</span>
                {#if step.error}<span class="trace-step-err">err</span>{/if}
              </div>
              <div class="trace-step-meta">
                <span>{step.computeUnits} CU</span>
                <span>{step.diffsCount} diffs</span>
                <span class="trace-step-scn">{step.scenario}</span>
              </div>
              {#if step.logsExcerpt && step.logsExcerpt.length > 0}
                <pre class="trace-step-logs">{step.logsExcerpt.slice(0, 5).join('\n')}</pre>
              {/if}
            </div>
          {/each}
        {:else}
          <SessionTimeline {contract} />
        {/if}
      {:else if activeTab === 'state'}
        {#if kind === 'solana' && program}
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

          {#if solanaUsers.length === 0}
            <div class="solana-empty">
              <div class="solana-empty-title">No users yet</div>
              <div class="solana-empty-hint">Create one above or run <code>users new &lt;name&gt;</code> in the terminal.</div>
            </div>
          {/if}
          {#each solanaUsers as u (u.name)}
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
          <StatePanel {contract} />
        {/if}
      {:else if contractDetail}
        <NodeInspector
          {selectedNode}
          {selectedPath}
          {funcPaths}
          {expandedFuncs}
          {seqExpanded}
          {mode}
          {seqAnalysis}
          contract={contractDetail}
          {lookupBlock}
          {onpathselect}
          {onsolanarun}
          {onexpandcfg}
        />
      {/if}
    </div>
  </div>

  <!-- Floating terminal (positions itself fixed, outside sidebar flow) -->
  <EmbeddedTerminal />
</div>

<style>
  .solana-empty {
    text-align: center;
    padding: 24px 12px;
  }
  .solana-empty-title {
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .solana-empty-hint {
    margin-top: 6px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--color-text-dim);
  }
  .solana-empty-hint code {
    background: var(--color-dark);
    padding: 1px 5px;
    border-radius: 3px;
  }
  .trace-step {
    background: var(--color-dark);
    border: 1px solid var(--color-border-subtle);
    border-radius: 6px;
    padding: 8px;
    margin: 6px 4px;
  }
  .trace-step-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .trace-step-idx {
    font-size: 10px;
    color: var(--color-text-dim);
    font-family: var(--font-mono, monospace);
  }
  .trace-step-name {
    font-family: var(--font-mono, monospace);
    font-weight: 700;
    color: var(--color-text);
  }
  .trace-step-err {
    font-size: 9px;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(220, 80, 80, 0.15);
    color: var(--color-danger);
  }
  .trace-step-meta {
    display: flex;
    gap: 10px;
    font-size: 10px;
    color: var(--color-text-dim);
    margin-top: 4px;
  }
  .trace-step-scn {
    margin-left: auto;
    color: var(--color-accent-hover);
  }
  .trace-step-logs {
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
    margin: 6px 4px 12px;
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
    margin: 6px 4px;
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
