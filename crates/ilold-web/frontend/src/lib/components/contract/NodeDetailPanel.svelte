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

  // Palette constants matching the parent
  const C = {
    ok: '#5a9a6a',
    danger: '#b05050',
    textMuted: '#6b7a8d',
  };

  function termColor(t: string): string {
    return t === 'Return' ? C.ok : t === 'Revert' ? C.danger : C.textMuted;
  }
</script>

<DraggablePanel
  title={selectedNode._funcName || selectedNode.label || ''}
  x={Math.min(window.innerWidth - 320, window.innerWidth - 20)} y={60} width={Math.min(310, window.innerWidth - 40)}
  onclose={onclose}
>
  <div class="detail">
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
          <div class="d-hint" style="color:#484f58">No state dependencies with previous function</div>
        {/if}
      {:else}
        <div class="d-hint" style="color:#484f58">No state dependencies in chain</div>
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
  .detail { padding: 8px; }
  .d-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 12px; color: #b8c4d4; }
  .d-label { color: #6b7a8d; }
  .d-hint { font-size: 11px; color: #5b9bd5; padding: 6px 0; font-style: italic; }
  .d-actions { padding: 6px 0; display: flex; flex-direction: column; gap: 4px; }
  .d-action-btn {
    background: #1a1a22; border: 1px solid #252530; color: #8bb8e8;
    padding: 6px 10px; border-radius: 4px; cursor: pointer;
    font-size: 11px; font-family: monospace; text-align: left;
  }
  .d-action-btn:hover { border-color: #5b9bd5; background: #1e1e28; }
  .d-section { font-size: 10px; color: #6b7a8d; text-transform: uppercase; letter-spacing: 0.5px; margin: 8px 0 4px; font-weight: 600; }
  .d-chain-step { font-size: 11px; color: #8bb8e8; font-weight: 600; margin: 6px 0 2px; padding-top: 4px; border-top: 1px solid #2a2d38; }
  .d-path-chain { font-size: 10px; color: #4a5568; padding: 4px 0; font-family: monospace; word-break: break-all; }

  .d-section-label { font-size: 10px; color: #4a5568; margin: 8px 0 4px; }

  /* Narrative panel */
  .narrative { margin-top: 6px; }
  .narr-label { font-size: 10px; color: #6b7a8d; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 4px; }
  .narr-sub { font-size: 9px; color: #4a5568; text-transform: uppercase; margin: 6px 0 2px; }
  .narr-condition {
    font-family: monospace; font-size: 11px;
    padding: 3px 8px; margin: 2px 0;
    background: #c49a4a12; border-left: 2px solid #c49a4a;
    color: #c49a4a; border-radius: 0 3px 3px 0;
  }

  /* Flow list */
  .flow-list { display: flex; flex-direction: column; gap: 0; }
  .flow-arrow { color: #252530; font-size: 10px; padding-left: 6px; line-height: 1; }
  .flow-step {
    font-family: monospace; font-size: 11px; color: #b8c4d4;
    padding: 4px 8px; border-radius: 4px;
    display: flex; align-items: center; gap: 5px;
    border-left: 2px solid #252530;
  }
  .flow-step.flow-entry { color: #8bb8e8; font-weight: 600; border-left-color: #5b9bd5; }
  .flow-step.flow-return { color: #5a9a6a; border-left-color: #5a9a6a; }
  .flow-step.flow-revert { color: #b05050; border-left-color: #b05050; }
  .flow-step.flow-check { color: #c49a4a; border-left-color: #c49a4a; background: #c49a4a08; }
  .flow-step.flow-call { color: #b8c4d4; border-left-color: #b05050; }
  .flow-step.flow-write { color: #6b7a8d; border-left-color: #5b9bd5; }
  .flow-step.flow-here { background: #5b9bd512; border-left-color: #5b9bd5; }
  .flow-badge {
    font-size: 9px; width: 14px; height: 14px;
    display: inline-flex; align-items: center; justify-content: center;
    border-radius: 3px; flex-shrink: 0;
  }
  .flow-badge.pass { background: #5a9a6a22; color: #5a9a6a; }
  .flow-badge.fail { background: #b0505022; color: #b05050; }
  .flow-badge.call { background: #b0505015; color: #b05050; }
  .flow-badge.write { background: #5b9bd515; color: #5b9bd5; }
  .flow-here-tag { color: #5b9bd5; font-size: 9px; margin-left: auto; }
  .d-path { display: flex; align-items: center; gap: 4px; padding: 3px 4px; border-radius: 3px; font-size: 11px; color: inherit; background: transparent; border: 1px solid transparent; cursor: pointer; width: 100%; text-align: left; font: inherit; }
  .d-path:hover { background: #121215; }
  .pid { color: #4a5568; font-weight: 600; }
  .pdepth { color: #4a5568; font-size: 10px; }
  .pb { font-size: 9px; padding: 1px 4px; border-radius: 6px; }
  .pb.ext { background: #b0505018; color: #c07070; }
  .d-path-selected { background: #1e2028; border-color: #5b9bd5; }

  .pd-item {
    font-family: monospace; font-size: 11px;
    padding: 2px 6px; border-radius: 3px; margin-bottom: 2px;
  }
  .pd-item.check { background: #c49a4a18; color: #c49a4a; }
  .pd-item.ext { background: #b0505018; color: #c07070; }
  .pd-item.wr { background: #5b9bd518; color: #8bb8e8; }
  .pd-item.ev { background: #5a9a6a18; color: #7aba8a; }
</style>
