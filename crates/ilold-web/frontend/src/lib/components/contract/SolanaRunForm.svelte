<script lang="ts">
  import type { InstructionDef, ProgramDetail } from '$lib/api/rest';

  let {
    program,
    ix,
    users,
    onsubmit,
  }: {
    program: ProgramDetail;
    ix: InstructionDef;
    users: { name: string; pubkey: string }[];
    onsubmit: (payload: {
      args: Record<string, any>;
      accounts: Record<string, string>;
      signers: string[];
    }) => Promise<void>;
  } = $props();

  let argValues = $state<Record<string, string>>({});
  let accountValues = $state<Record<string, string>>({});
  let signerValues = $state<Record<string, boolean>>({});
  let submitting = $state(false);
  let errorMsg = $state<string | null>(null);

  $effect(() => {
    const a: Record<string, string> = {};
    for (const arg of ix.args ?? []) a[arg.name] = '';
    argValues = a;

    const accs: Record<string, string> = {};
    const sgn: Record<string, boolean> = {};
    for (const acc of ix.accounts ?? []) {
      accs[acc.name] = '';
      if (acc.signer) sgn[acc.name] = true;
    }
    accountValues = accs;
    signerValues = sgn;
  });

  const visibleAccounts = $derived((ix.accounts ?? []).filter((a: any) => !a.address));
  const constantAccounts = $derived((ix.accounts ?? []).filter((a: any) => a.address));

  const NUMBER_INTS = new Set(['u8','u16','u32','u64','i8','i16','i32','i64','f32','f64']);
  const STRING_INTS = new Set(['u128','i128','u256','i256']);

  function coerceArg(raw: string, ty: any): any {
    if (typeof ty === 'string') {
      if (ty === 'bool') return raw === 'true' || raw === '1';
      if (NUMBER_INTS.has(ty)) {
        const n = Number(raw);
        return Number.isFinite(n) ? n : raw;
      }
      if (STRING_INTS.has(ty)) {
        return raw;
      }
    }
    return raw;
  }

  function describeType(ty: any): string {
    if (typeof ty === 'string') return ty;
    if (ty == null) return '?';
    if (typeof ty === 'object' && 'defined' in ty) {
      const d = (ty as any).defined;
      return typeof d === 'string' ? d : (d?.name ?? JSON.stringify(d));
    }
    return JSON.stringify(ty);
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    errorMsg = null;
    submitting = true;
    try {
      const args: Record<string, any> = {};
      for (const arg of ix.args ?? []) {
        const raw = argValues[arg.name] ?? '';
        if (raw === '') continue;
        args[arg.name] = coerceArg(raw, arg.type);
      }
      const accounts: Record<string, string> = {};
      for (const [k, v] of Object.entries(accountValues)) {
        if (v && v.trim() !== '') accounts[k] = v.trim();
      }
      const signers: string[] = [];
      for (const [k, on] of Object.entries(signerValues)) {
        if (!on) continue;
        const candidate = accountValues[k];
        if (candidate && users.some((u) => u.name === candidate)) {
          signers.push(candidate);
        }
      }
      await onsubmit({ args, accounts, signers });
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<form class="run-form" onsubmit={handleSubmit}>
  <div class="d-section-label">Args</div>
  {#if (ix.args ?? []).length === 0}
    <div class="run-empty">no args declared</div>
  {/if}
  {#each ix.args ?? [] as arg}
    <label class="run-row">
      <span class="run-label">{arg.name}</span>
      <input
        class="run-input"
        type="text"
        bind:value={argValues[arg.name]}
        placeholder={describeType(arg.type)}
      />
    </label>
  {/each}

  <div class="d-section-label">Accounts</div>
  {#each visibleAccounts as acc}
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
        placeholder={users.length > 0 ? 'user name or pubkey' : 'pubkey'}
      />
      {#if acc.signer}
        <label class="run-signer">
          <input type="checkbox" bind:checked={signerValues[acc.name]} />
          sign
        </label>
      {/if}
    </label>
  {/each}

  {#if constantAccounts.length > 0}
    <div class="run-auto-fill">
      Auto-filled: {constantAccounts.map((a: any) => a.name).join(', ')}
    </div>
  {/if}

  {#if users.length > 0}
    <div class="run-hint">Users: {users.map((u) => u.name).join(', ')}</div>
  {/if}
  {#if errorMsg}
    <div class="run-error">{errorMsg}</div>
  {/if}

  <button class="run-submit" type="submit" disabled={submitting}>
    {submitting ? 'Running…' : '▶ Execute'}
  </button>
  <div class="run-program">in {program.name}</div>
</form>

<style>
  .run-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 4px;
  }
  .d-section-label {
    font-size: 10px;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-top: 6px;
  }
  .run-empty {
    color: var(--color-text-dim);
    font-size: 11px;
    font-style: italic;
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
    padding: 5px 8px;
    color: var(--color-text);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
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
  .run-auto-fill {
    font-size: 10px;
    color: var(--color-text-dim);
    font-style: italic;
    background: var(--color-hover);
    border-radius: 4px;
    padding: 4px 6px;
  }
  .run-error {
    font-size: 11px;
    color: var(--color-danger);
    background: rgba(220, 80, 80, 0.1);
    padding: 6px 8px;
    border-radius: 4px;
  }
  .run-submit {
    margin-top: 8px;
    padding: 6px 12px;
    border-radius: 4px;
    border: 1px solid var(--color-accent);
    background: var(--color-accent);
    color: var(--color-dark);
    font-weight: 700;
    cursor: pointer;
  }
  .run-submit:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .run-program {
    font-size: 10px;
    color: var(--color-text-dim);
    text-align: center;
  }
</style>
