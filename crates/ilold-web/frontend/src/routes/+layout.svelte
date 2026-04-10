<script lang="ts">
  import { onMount } from 'svelte';
  import SearchPanel from '$lib/SearchPanel.svelte';
  import { toggleSearch } from '$lib/stores/search.svelte';
  import '../app.css';

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
