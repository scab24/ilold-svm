<script lang="ts">
  import { onMount } from 'svelte';
  import CommandPalette from '$lib/CommandPalette.svelte';
  import { togglePalette } from '$lib/stores/palette.svelte';
  import '../app.css';

  let { children } = $props();

  onMount(() => {
    function handleKeydown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        togglePalette();
      }
    }
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<CommandPalette />
{@render children()}
