<script lang="ts">
  import type { InstructionDef, ProgramDetail } from '$lib/api/rest';

  let {
    program,
    ix,
    users,
    onsubmit,
    oncancel,
  }: {
    program: ProgramDetail;
    ix: InstructionDef;
    users: { name: string; pubkey: string }[];
    onsubmit: (payload: {
      args: Record<string, any>;
      accounts: Record<string, string>;
      signers: string[];
    }) => Promise<void>;
    oncancel: () => void;
  } = $props();

  let argValues = $state<Record<string, string>>({});
  let accountValues = $state<Record<string, string>>({});
  let signerValues = $state<Record<string, boolean>>({});
  let submitting = $state(false);
  let errorMsg = $state<string | null>(null);

  $effect(() => {
    const initialArgs: Record<string, string> = {};
    for (const a of ix.args ?? []) initialArgs[a.name] = '';
    argValues = initialArgs;

    const initialAccs: Record<string, string> = {};
    const initialSigners: Record<string, boolean> = {};
    for (const acc of ix.accounts ?? []) {
      initialAccs[acc.name] = '';
      if (acc.signer) initialSigners[acc.name] = false;
    }
    accountValues = initialAccs;
    signerValues = initialSigners;
  });

  async function handleSubmit() {
    errorMsg = null;
    submitting = true;
    try {
      const args: Record<string, any> = {};
      for (const a of ix.args ?? []) {
        const raw = argValues[a.name] ?? '';
        if (raw === '') continue;
        args[a.name] = coerceArg(raw, a.ty);
      }
      const accounts: Record<string, string> = {};
      for (const [k, v] of Object.entries(accountValues)) {
        if (v && v.trim() !== '') accounts[k] = v.trim();
      }
      const signers: string[] = [];
      for (const [k, on] of Object.entries(signerValues)) {
        if (!on) continue;
        const userName = accountValues[k];
        if (userName && users.some((u) => u.name === userName)) {
          signers.push(userName);
        }
      }
      await onsubmit({ args, accounts, signers });
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }

  function coerceArg(raw: string, ty: any): any {
    if (typeof ty === 'string') {
      if (ty === 'bool') return raw === 'true' || raw === '1';
      if (['u8','u16','u32','u64','i8','i16','i32','i64'].includes(ty)) {
        const n = Number(raw);
        return Number.isFinite(n) ? n : raw;
      }
    }
    return raw;
  }

  function describeType(ty: any): string {
    if (typeof ty === 'string') return ty;
    if (ty == null) return '?';
    return JSON.stringify(ty);
  }
</script>

<div class="run-panel" role="dialog" aria-modal="true">
  <div class="run-panel-card">
    <div class="run-panel-header">
      <span class="run-panel-title">Run {ix.name}</span>
      <span class="run-panel-program">{program.name}</span>
      <button class="run-panel-close" onclick={oncancel} aria-label="Close">✕</button>
    </div>
    <div class="run-panel-body">
      {#if (ix.args ?? []).length > 0}
        <div class="run-section-label">Args</div>
        {#each ix.args ?? [] as arg}
          <label class="run-row">
            <span class="run-label">{arg.name}</span>
            <input
              class="run-input"
              type="text"
              bind:value={argValues[arg.name]}
              placeholder={describeType(arg.ty)}
            />
          </label>
        {/each}
      {/if}

      <div class="run-section-label">Accounts</div>
      {#each ix.accounts ?? [] as acc}
        <label class="run-row">
          <span class="run-label">
            {acc.name}
            {#if acc.signer}<span class="acc-flag">signer</span>{/if}
            {#if acc.writable}<span class="acc-flag">writable</span>{/if}
            {#if acc.pda}<span class="acc-flag pda">pda</span>{/if}
          </span>
          <input
            class="run-input"
            type="text"
            bind:value={accountValues[acc.name]}
            placeholder={users.length > 0 ? `user name or pubkey` : 'pubkey'}
          />
          {#if acc.signer}
            <label class="run-signer">
              <input type="checkbox" bind:checked={signerValues[acc.name]} />
              sign
            </label>
          {/if}
        </label>
      {/each}

      {#if users.length > 0}
        <div class="run-hint">
          Available users: {users.map((u) => u.name).join(', ')}
        </div>
      {/if}

      {#if errorMsg}
        <div class="run-error">{errorMsg}</div>
      {/if}
    </div>
    <div class="run-panel-footer">
      <button class="run-cancel" onclick={oncancel} disabled={submitting}>Cancel</button>
      <button class="run-submit" onclick={handleSubmit} disabled={submitting}>
        {submitting ? 'Running…' : 'Execute'}
      </button>
    </div>
  </div>
</div>

<style>
  .run-panel {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .run-panel-card {
    background: var(--color-hover);
    border: 1px solid var(--color-border-subtle);
    border-radius: 10px;
    width: 480px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    color: var(--color-text);
  }
  .run-panel-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .run-panel-title {
    font-weight: 700;
    font-family: var(--font-mono, monospace);
  }
  .run-panel-program {
    font-size: 11px;
    color: var(--color-text-muted);
  }
  .run-panel-close {
    margin-left: auto;
    background: transparent;
    border: 0;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .run-panel-body {
    padding: 12px 14px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .run-section-label {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--color-text-muted);
    letter-spacing: 0.08em;
    margin-top: 6px;
  }
  .run-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .run-label {
    font-size: 11px;
    font-family: var(--font-mono, monospace);
    color: var(--color-text);
  }
  .run-input {
    background: var(--color-dark);
    border: 1px solid var(--color-border-subtle);
    border-radius: 4px;
    padding: 6px 8px;
    color: var(--color-text);
    font-family: var(--font-mono, monospace);
    font-size: 12px;
  }
  .acc-flag {
    font-size: 9px;
    margin-left: 4px;
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--color-border-subtle);
    color: var(--color-text-muted);
  }
  .acc-flag.pda {
    background: rgba(196, 154, 74, 0.15);
    color: var(--color-warning);
  }
  .run-signer {
    font-size: 10px;
    color: var(--color-text-muted);
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .run-hint {
    font-size: 10px;
    color: var(--color-text-dim);
    font-style: italic;
  }
  .run-error {
    font-size: 11px;
    color: var(--color-danger);
    background: rgba(220, 80, 80, 0.1);
    padding: 6px 8px;
    border-radius: 4px;
  }
  .run-panel-footer {
    display: flex;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--color-border-subtle);
    justify-content: flex-end;
  }
  .run-cancel,
  .run-submit {
    padding: 6px 14px;
    border-radius: 4px;
    border: 1px solid var(--color-border-subtle);
    background: var(--color-hover);
    color: var(--color-text);
    cursor: pointer;
  }
  .run-submit {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-dark);
    font-weight: 600;
  }
  .run-cancel:disabled,
  .run-submit:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
