<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import SearchPanel from '$lib/SearchPanel.svelte';
  import { toggleSearch } from '$lib/stores/search';

  let { children } = $props();
  const isHome = false; // All pages are now fullscreen with their own topbar

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
    background: #181a20;
    color: #b8c4d4;
    overflow: hidden;
  }
  :global(a) { color: #5b9bd5; text-decoration: none; }
  :global(a:hover) { color: #8bb8e8; text-decoration: none; }
  :global(button) { font-family: inherit; }

  .app {
    min-height: 100vh;
    display: flex; flex-direction: column;
    overflow-y: auto;
  }

  header {
    background: #1e2028;
    border-bottom: 1px solid #2a2d38;
    padding: 12px 24px;
  }
  nav { display: flex; align-items: center; gap: 12px; }

  .logo { font-size: 20px; font-weight: 700; color: #b8c4d4; letter-spacing: -0.5px; }
  .logo:hover { text-decoration: none; }
  .subtitle { font-size: 13px; color: #6b7a8d; }

  .search-btn {
    margin-left: auto;
    background: #1e2028; border: 1px solid #2a2d38;
    color: #6b7a8d; padding: 5px 12px;
    border-radius: 6px; cursor: pointer;
    font-size: 12px; display: flex; align-items: center; gap: 6px;
  }
  .search-btn:hover { border-color: #5b9bd5; color: #b8c4d4; }
  .search-btn kbd {
    background: #181a20; padding: 1px 5px;
    border-radius: 3px; font-size: 10px; border: 1px solid #2a2d38;
  }

  main {
    flex: 1; padding: 24px;
    max-width: 1400px; margin: 0 auto;
    width: 100%; box-sizing: border-box;
  }
</style>
