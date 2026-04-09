<script lang="ts">
  import { onMount } from 'svelte';
  import SearchPanel from '$lib/SearchPanel.svelte';
  import { toggleSearch } from '$lib/stores/search';

  let { children } = $props();

  onMount(() => {
    function handleKeydown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        toggleSearch();
      }
    }
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<SearchPanel />
{@render children()}

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
    background: #121215;
    color: #b8c4d4;
    overflow: hidden;
  }
  :global(a) { color: #5b9bd5; text-decoration: none; }
  :global(a:hover) { color: #8bb8e8; text-decoration: none; }
  :global(button) { font-family: inherit; }
</style>
