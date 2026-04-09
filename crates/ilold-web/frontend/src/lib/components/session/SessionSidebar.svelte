<script lang="ts">
  import CommandBar from './CommandBar.svelte';
  import SessionTimeline from './SessionTimeline.svelte';
  import StatePanel from './StatePanel.svelte';

  let { contract }: { contract: string } = $props();

  let open = $state(true);
  let activeTab: 'timeline' | 'state' = $state('timeline');
</script>

<div class="session-sidebar" class:collapsed={!open}>
  <button class="toggle" onclick={() => open = !open}>{open ? '▸' : '◂'}</button>

  <div class="content" class:hidden={!open}>
    <div class="tabs">
      <button class="tab" class:active={activeTab === 'timeline'} onclick={() => activeTab = 'timeline'}>Timeline</button>
      <button class="tab" class:active={activeTab === 'state'} onclick={() => activeTab = 'state'}>State</button>
    </div>

    <div class="panel-area">
      {#if activeTab === 'timeline'}
        <SessionTimeline {contract} />
      {:else}
        <StatePanel {contract} />
      {/if}
    </div>

    <div class="cmd-area">
      <CommandBar {contract} />
    </div>
  </div>
</div>

<style>
  .session-sidebar {
    width: 360px; flex-shrink: 0;
    background: #121215; border-left: 1px solid #252530;
    display: flex; flex-direction: column;
    position: relative; height: 100%;
  }
  .session-sidebar.collapsed { width: 28px; }

  .toggle {
    position: absolute; left: -1px; top: 8px;
    background: #18181e; border: 1px solid #252530;
    border-right: none; border-radius: 4px 0 0 4px;
    color: #6b7a8d; cursor: pointer; padding: 4px 3px;
    font-size: 10px; z-index: 5;
  }
  .toggle:hover { color: #8bb8e8; }

  .tabs {
    display: flex; border-bottom: 1px solid #252530;
    padding: 0 4px;
    margin-bottom: 4px;
  }
  .tab {
    flex: 1; padding: 6px 0;
    background: none; border: none; border-bottom: 2px solid transparent;
    color: #6b7a8d; font-size: 10px; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.5px;
    cursor: pointer;
  }
  .tab:hover { color: #b8c4d4; }
  .tab.active { color: #5b9bd5; border-bottom-color: #5b9bd5; }

  .content {
    display: flex; flex-direction: column;
    flex: 1; min-height: 0;
  }
  .content.hidden { display: none; }

  .panel-area { flex: 1; overflow-y: auto; min-height: 0; padding: 0 4px; }
  .cmd-area {
    border-top: 2px solid #252530;
    max-height: 40%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
