<script lang="ts">
  import CommandBar from './CommandBar.svelte';
  import SessionTimeline from './SessionTimeline.svelte';
  import StatePanel from './StatePanel.svelte';

  let { contract }: { contract: string } = $props();

  let open = $state(true);
  let activeTab: 'timeline' | 'state' = $state('timeline');
</script>

<div
  class="flex flex-col flex-shrink-0 bg-dark border-l border-border relative h-full"
  class:w-[360px]={open}
  class:w-[28px]={!open}
>
  <button
    class="absolute -left-px top-2 bg-surface border border-border border-r-0 rounded-l-[4px] text-text-muted cursor-pointer px-[3px] py-1 text-[10px] z-5 hover:text-accent-hover"
    onclick={() => open = !open}
  >
    {open ? '▸' : '◂'}
  </button>

  <div class="flex flex-col flex-1 min-h-0" class:hidden={!open}>
    <div class="flex border-b border-border px-1 mb-1">
      <button
        class="flex-1 py-1.5 bg-transparent border-none border-b-2 text-[10px] font-semibold uppercase tracking-[0.5px] cursor-pointer {activeTab === 'timeline' ? 'text-accent border-b-accent' : 'text-text-muted border-b-transparent hover:text-text'}"
        onclick={() => activeTab = 'timeline'}
      >
        Timeline
      </button>
      <button
        class="flex-1 py-1.5 bg-transparent border-none border-b-2 text-[10px] font-semibold uppercase tracking-[0.5px] cursor-pointer {activeTab === 'state' ? 'text-accent border-b-accent' : 'text-text-muted border-b-transparent hover:text-text'}"
        onclick={() => activeTab = 'state'}
      >
        State
      </button>
    </div>

    <div class="flex-1 overflow-y-auto min-h-0 px-1">
      {#if activeTab === 'timeline'}
        <SessionTimeline {contract} />
      {:else}
        <StatePanel {contract} />
      {/if}
    </div>

    <div class="border-t-2 border-border max-h-[40%] flex flex-col overflow-hidden">
      <CommandBar {contract} />
    </div>
  </div>
</div>
