<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import SearchPanel from '$lib/SearchPanel.svelte';
  import { toggleSearch } from '$lib/stores/search';

  let { children } = $props();
  const isHome = $derived(page.url.pathname === '/');

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

{#if isHome}
  <div class="app">
    <header>
      <nav>
        <a href="/" class="logo">ilold</a>
        <span class="subtitle">execution path analyzer</span>
        <button class="search-btn" onclick={toggleSearch}>🔍 Search <kbd>⌘K</kbd></button>
      </nav>
    </header>
    <main>
      {@render children()}
    </main>
  </div>
{:else}
  {@render children()}
{/if}

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
    background: #0d1117;
    color: #c9d1d9;
    overflow: hidden;
  }
  :global(a) { color: #58a6ff; text-decoration: none; }
  :global(a:hover) { text-decoration: underline; }
  :global(button) { font-family: inherit; }

  .app {
    min-height: 100vh;
    display: flex; flex-direction: column;
    overflow-y: auto;
  }

  header {
    background: #161b22;
    border-bottom: 1px solid #30363d;
    padding: 12px 24px;
  }
  nav { display: flex; align-items: center; gap: 12px; }

  .logo { font-size: 20px; font-weight: 700; color: #f0f6fc; letter-spacing: -0.5px; }
  .logo:hover { text-decoration: none; }
  .subtitle { font-size: 13px; color: #8b949e; }

  .search-btn {
    margin-left: auto;
    background: #21262d; border: 1px solid #30363d;
    color: #8b949e; padding: 5px 12px;
    border-radius: 6px; cursor: pointer;
    font-size: 12px; display: flex; align-items: center; gap: 6px;
  }
  .search-btn:hover { border-color: #58a6ff; color: #c9d1d9; }
  .search-btn kbd {
    background: #161b22; padding: 1px 5px;
    border-radius: 3px; font-size: 10px; border: 1px solid #30363d;
  }

  main {
    flex: 1; padding: 24px;
    max-width: 1400px; margin: 0 auto;
    width: 100%; box-sizing: border-box;
  }
</style>
