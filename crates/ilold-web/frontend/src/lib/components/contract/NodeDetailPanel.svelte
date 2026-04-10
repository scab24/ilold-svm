<script lang="ts">
  import DraggablePanel from '$lib/DraggablePanel.svelte';
  import Collapsible from '$lib/Collapsible.svelte';

  interface Props {
    selectedNode: any;
    selectedPath: any;
    funcPaths: Record<string, any>;
    expandedFuncs: Set<string>;
    seqExpanded: Map<string, boolean>;
    mode: 'cfg' | 'sequences';
    seqAnalysis: any;
    contract: { name: string; functions?: any[] };
    lookupBlock: (blockId: string) => { statements: string[]; node_type: string } | null;
    onclose: () => void;
    onpathselect: (funcName: string, path: any) => void;
    onexpandcfg: (funcName: string, nodeId?: string) => void;
  }

  let {
    selectedNode,
    selectedPath,
    funcPaths,
    expandedFuncs,
    seqExpanded,
    mode,
    seqAnalysis,
    contract,
    lookupBlock,
    onclose,
    onpathselect,
    onexpandcfg,
  }: Props = $props();

  function termColor(t: string): string {
    return t === 'Return' ? 'var(--color-success)' : t === 'Revert' ? 'var(--color-danger)' : 'var(--color-text-muted)';
  }
</script>

<DraggablePanel
  title={selectedNode._funcName || selectedNode.label || ''}
  x={Math.min(window.innerWidth - 320, window.innerWidth - 20)} y={60} width={Math.min(310, window.innerWidth - 40)}
  onclose={onclose}
>
  <div class="p-2">
    {#if selectedNode._type === 'function'}
      <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.is_external ? 'External' : 'Internal'}</span></div>
      {#if !selectedNode.is_external}
        <div class="d-actions">
          <button class="d-action-btn" onclick={() => onexpandcfg(selectedNode.label, selectedNode.id)}>
            {expandedFuncs.has(selectedNode.label) ? '▼ Collapse CFG' : '▶ Expand CFG'}
          </button>
          {#if mode === 'sequences'}
            <div class="d-hint">Click → expand · Shift+click → branch</div>
          {/if}
        </div>
      {/if}

      {#if funcPaths[selectedNode.label]}
        {@const fp = funcPaths[selectedNode.label]}
        <div class="d-row"><span class="d-label">Paths</span><span>{fp.stats.total_paths} ({fp.stats.happy_paths} return, {fp.stats.revert_paths} revert)</span></div>

        <Collapsible title="Paths" count={fp.stats.total_paths} open={true}>
          {#each fp.paths as path}
            <button
              class="d-path"
              class:d-path-selected={selectedPath?.id === path.id}
              onclick={() => onpathselect(selectedNode.label, path)}
            >
              <span class="pid">#{path.id}</span>
              <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
              <span class="pdepth">{path.nodes.length} steps</span>
              {#if path.annotations.external_calls.length > 0}
                <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
              {/if}
            </button>
          {/each}
        </Collapsible>

        {#if selectedPath}
          {@const ann = selectedPath.annotations}
          {@const pFunc = selectedNode.label}
          <div class="narrative">
            {#if ann.require_checks.length > 0}
              <div class="narr-label">Conditions required</div>
              {#each ann.require_checks as c}
                <div class="narr-condition">{c}</div>
              {/each}
            {/if}

            <div class="narr-label" style="margin-top:8px">Execution flow</div>
            <div class="flow-list">
              {#each selectedPath.nodes as step, i}
                {@const blockData = lookupBlock(`cfg:${pFunc}:b${step.block_id}`)}
                {@const stmts = blockData?.statements || []}
                {@const kind = blockData?.node_type || ''}
                {@const isLast = i === selectedPath.nodes.length - 1}
                {@const branchKind = typeof step.branch_taken === 'string' ? step.branch_taken : step.branch_taken?.kind || ''}
                {#if kind === 'Entry'}
                  <div class="flow-step flow-entry">{pFunc}()</div>
                {:else if kind === 'Return'}
                  <div class="flow-step flow-return">return</div>
                {:else if kind === 'Revert'}
                  <div class="flow-step flow-revert">revert</div>
                {:else}
                  {#each stmts as s}
                    {@const isRequire = s.startsWith('require(') || s.startsWith('require (')}
                    {@const isCall = s.includes('.') && s.includes('(') && !isRequire}
                    {@const isWrite = s.includes('=') && !s.includes('==') && !isCall}
                    <div
                      class="flow-step"
                      class:flow-check={isRequire}
                      class:flow-call={isCall}
                      class:flow-write={isWrite}
                    >
                      {#if branchKind === 'True' && isRequire}
                        <span class="flow-badge pass">✓</span>
                      {:else if branchKind === 'False' && isRequire}
                        <span class="flow-badge fail">✗</span>
                      {:else if isCall}
                        <span class="flow-badge call">→</span>
                      {:else if isWrite}
                        <span class="flow-badge write">✏</span>
                      {/if}
                      {s}
                    </div>
                  {/each}
                {/if}
                {#if !isLast}
                  <div class="flow-arrow">│</div>
                {/if}
              {/each}
            </div>

            {#if ann.external_calls.length > 0 || ann.state_writes.length > 0 || ann.events_emitted.length > 0}
              <Collapsible title="Side effects" count={ann.external_calls.length + ann.state_writes.length + ann.events_emitted.length} open={false}>
                {#if ann.external_calls.length > 0}
                  <div class="narr-sub">Calls</div>
                  {#each ann.external_calls as c}
                    <div class="pd-item ext">{c.target}.{c.function}()</div>
                  {/each}
                {/if}
                {#if ann.state_writes.length > 0}
                  <div class="narr-sub">Writes</div>
                  {#each ann.state_writes as w}
                    <div class="pd-item wr">{w}</div>
                  {/each}
                {/if}
                {#if ann.events_emitted.length > 0}
                  <div class="narr-sub">Emits</div>
                  {#each ann.events_emitted as e}
                    <div class="pd-item ev">{e}</div>
                  {/each}
                {/if}
              </Collapsible>
            {/if}
          </div>
        {:else}
          <div class="d-hint">Click a path to see its execution flow</div>
        {/if}
      {/if}
    {:else if selectedNode._type === 'seq-next'}
      {@const nodeId = selectedNode.id || ''}
      {@const pathParts = (nodeId.includes('::') ? nodeId.split('::')[1] : nodeId).split('→').map((s: string) => s.replace(/:b\d+$/, ''))}
      <div class="d-row"><span class="d-label">Function</span><span>{selectedNode._funcName || selectedNode.label}</span></div>
      <div class="d-row"><span class="d-label">Paths</span><span>{selectedNode.pathCount}</span></div>
      <div class="d-row"><span class="d-label">Type</span><span>{selectedNode.readOnly ? 'Read-only (view)' : 'State-changing'}</span></div>
      {#if pathParts.length > 1}
        <div class="d-path-chain">{pathParts.join(' → ')}</div>
      {/if}
      <div class="d-actions">
        <div class="d-hint">Click → expand · Shift+click → branch</div>
        {#if contract?.functions?.some(f => f.name === (selectedNode._funcName || selectedNode.label))}
          <button class="d-action-btn" onclick={() => onexpandcfg(selectedNode._funcName || selectedNode.label, selectedNode.id)}>
            ▶ Expand CFG
          </button>
        {/if}
      </div>

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
          <div class="d-hint" style="color:var(--color-text-dim)">No state dependencies with previous function</div>
        {/if}
      {:else}
        <div class="d-hint" style="color:var(--color-text-dim)">No state dependencies in chain</div>
      {/if}
    {:else if selectedNode._type === 'block'}
      {@const parentFunc = selectedNode._parentFunc || ''}
      {@const paths = funcPaths[parentFunc]?.paths || []}
      {@const passingPaths = paths.filter((p: any) => p.nodes.some((n: any) => `cfg:${parentFunc}:b${n.block_id}` === selectedNode.id))}
      <div class="d-row"><span class="d-label">Function</span><span>{parentFunc}</span></div>
      <div class="d-row"><span class="d-label">Reachable via</span><span>{passingPaths.length} of {paths.length} paths</span></div>

      {#if passingPaths.length > 0}
        <div class="d-section-label">Select a path to explore</div>
        {#each passingPaths as path}
          <button
            class="d-path"
            class:d-path-selected={selectedPath?.id === path.id}
            onclick={() => onpathselect(parentFunc, path)}
          >
            <span class="pid">#{path.id}</span>
            <span style="color:{termColor(path.terminal)};font-weight:600">{path.terminal}</span>
            <span class="pdepth">{path.nodes.length} steps</span>
            {#if path.annotations.external_calls.length > 0}
              <span class="pb ext">⚡{path.annotations.external_calls.length}</span>
            {/if}
          </button>
        {/each}
      {/if}

      {#if selectedPath}
        {@const currentBlockIdx = selectedPath.nodes.findIndex((n: any) => `cfg:${parentFunc}:b${n.block_id}` === selectedNode.id)}
        {@const routeToHere = currentBlockIdx >= 0 ? selectedPath.nodes.slice(0, currentBlockIdx + 1) : []}
        {@const ann = selectedPath.annotations}

        <div class="narrative">
          {#if ann.require_checks.length > 0}
            <div class="narr-label">Conditions required</div>
            {#each ann.require_checks as c}
              <div class="narr-condition">{c}</div>
            {/each}
          {/if}

          {#if routeToHere.length > 0}
            <div class="narr-label" style="margin-top:8px">Execution flow</div>
            <div class="flow-list">
              {#each routeToHere as step, i}
                {@const blockData = lookupBlock(`cfg:${parentFunc}:b${step.block_id}`)}
                {@const stmts = blockData?.statements || []}
                {@const kind = blockData?.node_type || ''}
                {@const isHere = i === routeToHere.length - 1}
                {@const branchKind = typeof step.branch_taken === 'string' ? step.branch_taken : step.branch_taken?.kind || ''}
                {#if kind === 'Entry'}
                  <div class="flow-step flow-entry">{parentFunc}()</div>
                {:else if kind === 'Return'}
                  <div class="flow-step flow-return" class:flow-here={isHere}>return {isHere ? '← here' : ''}</div>
                {:else if kind === 'Revert'}
                  <div class="flow-step flow-revert" class:flow-here={isHere}>revert {isHere ? '← here' : ''}</div>
                {:else}
                  {#each stmts as s}
                    {@const isRequire = s.startsWith('require(') || s.startsWith('require (')}
                    {@const isCall = s.includes('.') && s.includes('(') && !isRequire}
                    {@const isWrite = s.includes('=') && !s.includes('==') && !isCall}
                    <div
                      class="flow-step"
                      class:flow-check={isRequire}
                      class:flow-call={isCall}
                      class:flow-write={isWrite}
                      class:flow-here={isHere}
                    >
                      {#if branchKind === 'True' && isRequire}
                        <span class="flow-badge pass">✓</span>
                      {:else if branchKind === 'False' && isRequire}
                        <span class="flow-badge fail">✗</span>
                      {:else if isCall}
                        <span class="flow-badge call">→</span>
                      {:else if isWrite}
                        <span class="flow-badge write">✏</span>
                      {/if}
                      {s}
                      {#if isHere}<span class="flow-here-tag">← here</span>{/if}
                    </div>
                  {/each}
                {/if}
                {#if !isHere && i < routeToHere.length - 1}
                  <div class="flow-arrow">│</div>
                {/if}
              {/each}
            </div>
          {/if}

          {#if ann.external_calls.length > 0 || ann.state_writes.length > 0 || ann.events_emitted.length > 0}
            <Collapsible title="Side effects" count={ann.external_calls.length + ann.state_writes.length + ann.events_emitted.length} open={false}>
              {#if ann.external_calls.length > 0}
                <div class="narr-sub">Calls</div>
                {#each ann.external_calls as c}
                  <div class="pd-item ext">{c.target}.{c.function}()</div>
                {/each}
              {/if}
              {#if ann.state_writes.length > 0}
                <div class="narr-sub">Writes</div>
                {#each ann.state_writes as w}
                  <div class="pd-item wr">{w}</div>
                {/each}
              {/if}
              {#if ann.events_emitted.length > 0}
                <div class="narr-sub">Emits</div>
                {#each ann.events_emitted as e}
                  <div class="pd-item ev">{e}</div>
                {/each}
              {/if}
            </Collapsible>
          {/if}
        </div>
      {:else}
        <div class="d-hint">Click a path above to see the execution flow</div>
      {/if}
    {/if}
  </div>
</DraggablePanel>

<style>
  .d-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 12px; color: var(--color-text); }
  .d-label { color: var(--color-text-muted); }
  .d-hint { font-size: 11px; color: var(--color-accent); padding: 6px 0; font-style: italic; }
  .d-actions { padding: 6px 0; display: flex; flex-direction: column; gap: 4px; }
  .d-action-btn {
    background: var(--color-surface-alt); border: 1px solid var(--color-border); color: var(--color-accent-hover);
    padding: 6px 10px; border-radius: 4px; cursor: pointer;
    font-size: 11px; font-family: monospace; text-align: left;
  }
  .d-action-btn:hover { border-color: var(--color-accent); background: var(--color-hover); }
  .d-section { font-size: 10px; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; font-weight: 600; }
  .d-chain-step { font-size: 11px; color: var(--color-accent-hover); font-weight: 600; margin: 6px 0 2px; padding-top: 4px; border-top: 1px solid var(--color-border-subtle); }
  .d-path-chain { font-size: 10px; color: var(--color-text-dim); padding: 4px 0; font-family: monospace; word-break: break-all; }

  .d-section-label { font-size: 10px; color: var(--color-text-dim); margin: 8px 0 4px; }

  /* Narrative panel */
  .narrative { margin-top: 6px; }
  .narr-label { font-size: 10px; color: var(--color-text-muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 4px; }
  .narr-sub { font-size: 9px; color: var(--color-text-dim); text-transform: uppercase; margin: 6px 0 2px; }
  .narr-condition {
    font-family: monospace; font-size: 11px;
    padding: 3px 8px; margin: 2px 0;
    background: color-mix(in srgb, var(--color-warning) 7%, transparent);
    border-left: 2px solid var(--color-warning);
    color: var(--color-warning); border-radius: 0 3px 3px 0;
  }

  /* Flow list */
  .flow-list { display: flex; flex-direction: column; gap: 0; }
  .flow-arrow { color: var(--color-border); font-size: 10px; padding-left: 6px; line-height: 1; }
  .flow-step {
    font-family: monospace; font-size: 11px; color: var(--color-text);
    padding: 4px 8px; border-radius: 4px;
    display: flex; align-items: center; gap: 5px;
    border-left: 2px solid var(--color-border);
  }
  .flow-step.flow-entry { color: var(--color-accent-hover); font-weight: 600; border-left-color: var(--color-accent); }
  .flow-step.flow-return { color: var(--color-success); border-left-color: var(--color-success); }
  .flow-step.flow-revert { color: var(--color-danger); border-left-color: var(--color-danger); }
  .flow-step.flow-check { color: var(--color-warning); border-left-color: var(--color-warning); background: color-mix(in srgb, var(--color-warning) 3%, transparent); }
  .flow-step.flow-call { color: var(--color-text); border-left-color: var(--color-danger); }
  .flow-step.flow-write { color: var(--color-text-muted); border-left-color: var(--color-accent); }
  .flow-step.flow-here { background: color-mix(in srgb, var(--color-accent) 7%, transparent); border-left-color: var(--color-accent); }
  .flow-badge {
    font-size: 9px; width: 14px; height: 14px;
    display: inline-flex; align-items: center; justify-content: center;
    border-radius: 3px; flex-shrink: 0;
  }
  .flow-badge.pass { background: color-mix(in srgb, var(--color-success) 13%, transparent); color: var(--color-success); }
  .flow-badge.fail { background: color-mix(in srgb, var(--color-danger) 13%, transparent); color: var(--color-danger); }
  .flow-badge.call { background: color-mix(in srgb, var(--color-danger) 8%, transparent); color: var(--color-danger); }
  .flow-badge.write { background: color-mix(in srgb, var(--color-accent) 8%, transparent); color: var(--color-accent); }
  .flow-here-tag { color: var(--color-accent); font-size: 9px; margin-left: auto; }
  .d-path { display: flex; align-items: center; gap: 4px; padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit; background: transparent; border: 1px solid transparent; cursor: pointer; width: 100%; text-align: left; font: inherit; }
  .d-path:hover { background: var(--color-dark); }
  .pid { color: var(--color-text-dim); font-weight: 600; }
  .pdepth { color: var(--color-text-dim); font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: color-mix(in srgb, var(--color-danger) 9%, transparent); color: var(--color-danger-light); }
  .d-path-selected { background: var(--color-hover); border-color: var(--color-accent); }

  .pd-item {
    font-family: monospace; font-size: 11px;
    padding: 2px 6px; border-radius: 3px; margin-bottom: 2px;
  }
  .pd-item.check { background: color-mix(in srgb, var(--color-warning) 9%, transparent); color: var(--color-warning); }
  .pd-item.ext { background: color-mix(in srgb, var(--color-danger) 9%, transparent); color: var(--color-danger-light); }
  .pd-item.wr { background: color-mix(in srgb, var(--color-accent) 9%, transparent); color: var(--color-accent-hover); }
  .pd-item.ev { background: color-mix(in srgb, var(--color-success) 9%, transparent); color: var(--color-success-light); }
</style>
