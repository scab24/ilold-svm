<script lang="ts">
  import { tick } from 'svelte';
  import { postCommand, type SessionCommand } from '$lib/api/session';
  import { formatAccess } from '$lib/utils/access';

  let { contract }: { contract: string } = $props();

  // ── State ──────────────────────────────────────────────────────────────────
  let input = $state('');
  let lines: Array<{ text: string; kind: 'cmd' | 'ok' | 'err' }> = $state([]);
  let history: string[] = $state([]);
  let histIdx = $state(-1);
  let busy = $state(false);

  let outputEl: HTMLDivElement | undefined = $state(undefined);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  // ── Auto-scroll helper ──────────────────────────────────────────────────────
  async function scrollToBottom() {
    await tick();
    if (outputEl) outputEl.scrollTop = outputEl.scrollHeight;
  }

  // ── Auto-focus on mount ────────────────────────────────────────────────────
  $effect(() => {
    inputEl?.focus();
  });

  // ── Parse shortcuts ────────────────────────────────────────────────────────
  function parse(raw: string): SessionCommand | null {
    const trimmed = raw.trim();
    if (!trimmed) return null;

    if (trimmed === 'b') return 'Back';
    if (trimmed === 'x') return 'Clear';
    if (trimmed === 's') return 'State';
    if (trimmed === 'f') return 'Functions';
    if (trimmed === 'fa') return 'FunctionsAll';
    if (trimmed === 'sv') return 'StateVarsAll';
    if (trimmed === 'session') return 'Session';
    if (trimmed === 'export') return 'Export';
    if (trimmed === 'save') return 'SaveSession';

    if (trimmed.startsWith('c ')) {
      const func = trimmed.slice(2).trim();
      return func ? { Call: { func } } : null;
    }
    if (trimmed.startsWith('w ')) {
      const variable = trimmed.slice(2).trim();
      return variable ? { Who: { variable } } : null;
    }

    return null;
  }

  // ── Helpers ────────────────────────────────────────────────────────────────
  function formatFuncList(funcs: any[]): { text: string; kind: 'ok' }[] {
    return funcs.map((f: any) => ({
      text: `  ${f.name ?? '?'}  ${f.access ? formatAccess(f.access) : ''}  ${f.writes_state ? '[writes]' : ''}`,
      kind: 'ok' as const,
    }));
  }

  // ── Format result ──────────────────────────────────────────────────────────
  function formatResult(data: unknown): Array<{ text: string; kind: 'ok' | 'err' }> {
    if (!data || typeof data !== 'object') {
      return [{ text: JSON.stringify(data), kind: 'ok' }];
    }

    const d = data as Record<string, any>;

    // StepAdded
    if ('StepAdded' in d) {
      const s = d.StepAdded;
      const stateVars = s.state_changed?.length ? s.state_changed.join(', ') : 'none';
      return [{ text: `\u2192 ${s.function} [${formatAccess(s.access)}] (state: ${stateVars})`, kind: 'ok' }];
    }

    // StepRemoved
    if ('StepRemoved' in d) {
      const s = d.StepRemoved;
      return [{ text: `\u2190 Back (${s.remaining} steps left)`, kind: 'ok' }];
    }

    // Cleared
    if ('Cleared' in d) {
      return [{ text: 'Session cleared', kind: 'ok' }];
    }

    // StateView
    if ('StateView' in d) {
      const vars: any[] = d.StateView.summary ?? d.StateView ?? [];
      if (!Array.isArray(vars) || vars.length === 0) return [{ text: 'No state variables', kind: 'ok' }];
      return vars.map((v: any) => {
        const name = v.variable ?? '?';
        const lastChange = v.changes?.length > 0 ? v.changes[v.changes.length - 1] : 'no changes';
        return { text: `  ${name}: ${lastChange}`, kind: 'ok' as const };
      });
    }

    // FunctionList
    if ('FunctionList' in d) {
      const funcs: any[] = d.FunctionList.functions ?? d.FunctionList ?? [];
      if (!Array.isArray(funcs) || funcs.length === 0) return [{ text: 'No functions', kind: 'ok' }];
      return formatFuncList(funcs);
    }

    // VariableInfo (Who)
    if ('VariableInfo' in d) {
      const v = d.VariableInfo;
      const out: Array<{ text: string; kind: 'ok' }> = [];
      out.push({ text: `Variable: ${v.variable ?? v.name ?? '?'}`, kind: 'ok' });
      if (v.writers?.length) out.push({ text: `  Writers: ${v.writers.join(', ')}`, kind: 'ok' });
      if (v.readers?.length) out.push({ text: `  Readers: ${v.readers.join(', ')}`, kind: 'ok' });
      return out;
    }

    // SessionView
    if ('SessionView' in d) {
      const s = d.SessionView;
      return [{ text: `Session: ${s.contract} | ${s.steps?.length ?? 0} steps | ${s.findings_count ?? 0} findings`, kind: 'ok' }];
    }

    // FunctionListAll
    if ('FunctionListAll' in d) {
      const funcs: any[] = d.FunctionListAll?.functions ?? d.functions ?? [];
      if (!Array.isArray(funcs) || funcs.length === 0) return [{ text: 'No functions', kind: 'ok' }];
      return formatFuncList(funcs);
    }

    // StateVarListAll
    if ('StateVarListAll' in d) {
      const vars: any[] = d.StateVarListAll?.state_vars ?? d.state_vars ?? [];
      if (!Array.isArray(vars) || vars.length === 0) return [{ text: 'No state variables', kind: 'ok' }];
      return vars.map((v: any) => ({
        text: `  ${v.name ?? '?'}  ${v.type ?? ''}`,
        kind: 'ok' as const,
      }));
    }

    // SessionSaved
    if ('SessionSaved' in d) {
      return [{ text: `Session saved (${d.SessionSaved?.json?.length ?? 0} bytes)`, kind: 'ok' }];
    }

    // Error
    if ('Error' in d) {
      return [{ text: d.Error.message ?? d.Error, kind: 'err' }];
    }

    // Exported
    if ('Exported' in d) {
      return [{ text: `Exported: ${JSON.stringify(d.Exported)}`, kind: 'ok' }];
    }

    // Fallback
    return [{ text: JSON.stringify(data, null, 2), kind: 'ok' }];
  }

  // ── Submit ─────────────────────────────────────────────────────────────────
  async function submit() {
    const raw = input.trim();
    if (!raw || busy) return;

    const cmd = parse(raw);
    if (!cmd) {
      lines.push({ text: `? Unknown: "${raw}"`, kind: 'err' });
      scrollToBottom();
      input = '';
      return;
    }

    lines.push({ text: `> ${raw}`, kind: 'cmd' });
    if (lines.length > 500) lines.splice(0, lines.length - 500);
    history.push(raw);
    if (history.length > 100) history.splice(0, history.length - 100);
    histIdx = -1;
    input = '';
    busy = true;
    scrollToBottom();

    try {
      const result = await postCommand(cmd, contract);
      lines.push(...formatResult(result));
    } catch (e: any) {
      lines.push({ text: e.message ?? String(e), kind: 'err' });
    } finally {
      busy = false;
      scrollToBottom();
      inputEl?.focus();
    }
  }

  // ── Key handler ────────────────────────────────────────────────────────────
  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      submit();
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (history.length === 0) return;
      if (histIdx === -1) histIdx = history.length - 1;
      else if (histIdx > 0) histIdx--;
      input = history[histIdx];
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (histIdx === -1) return;
      if (histIdx < history.length - 1) { histIdx++; input = history[histIdx]; }
      else { histIdx = -1; input = ''; }
    }
  }
</script>

<div class="cmd-bar">
  <div class="cmd-output" bind:this={outputEl}>
    {#each lines as line}
      <div class="cmd-line" class:cmd={line.kind === 'cmd'} class:err={line.kind === 'err'}>{line.text}</div>
    {/each}
    {#if lines.length === 0}
      <div class="cmd-hint">c &lt;func&gt; | b | x | s | f | fa | sv | w &lt;var&gt; | session | export</div>
    {/if}
  </div>
  <div class="cmd-input-row">
    <span class="cmd-prompt">{busy ? '...' : '>'}</span>
    <input
      class="cmd-input"
      type="text"
      bind:value={input}
      bind:this={inputEl}
      onkeydown={onkey}
      placeholder="command"
      disabled={busy}
      spellcheck="false"
      autocomplete="off"
    />
  </div>
</div>

<style>
  .cmd-bar {
    display: flex; flex-direction: column;
    background: #121215; border: 1px solid #252530;
    border-radius: 6px; overflow: hidden;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 12px; line-height: 1.5;
    height: 100%;
  }
  .cmd-output {
    flex: 1; overflow-y: auto; padding: 8px 10px;
    min-height: 0; max-height: 200px;
  }
  .cmd-output::-webkit-scrollbar { width: 4px; }
  .cmd-output::-webkit-scrollbar-thumb { background: #333; border-radius: 2px; }
  .cmd-line { white-space: pre-wrap; word-break: break-all; color: #8b95a5; line-height: 1.6; margin-bottom: 1px; }
  .cmd-line.cmd { color: #5b9bd5; }
  .cmd-line.err { color: #d05050; }
  .cmd-hint { color: #3a3f4a; font-style: italic; }
  .cmd-input-row {
    display: flex; align-items: center; gap: 6px;
    padding: 6px 10px; border-top: 1px solid #252530;
    background: #18181e;
  }
  .cmd-prompt { color: #5b9bd5; flex-shrink: 0; user-select: none; }
  .cmd-input {
    flex: 1; background: transparent; border: none; outline: none;
    color: #c8d0dc; font-family: inherit; font-size: inherit;
    caret-color: #5b9bd5;
  }
  .cmd-input::placeholder { color: #3a3f4a; }
  .cmd-input:disabled { opacity: 0.5; }
</style>
