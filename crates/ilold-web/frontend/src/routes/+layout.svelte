<script lang="ts">
  import { page } from '$app/state';
  let { children } = $props();

  // Only show the global header on the home page.
  // Contract and function views have their own fullscreen topbar.
  const isHome = $derived(page.url.pathname === '/');
</script>

{#if isHome}
  <div class="app">
    <header>
      <nav>
        <a href="/" class="logo">ilold</a>
        <span class="subtitle">execution path analyzer</span>
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
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  header {
    background: #161b22;
    border-bottom: 1px solid #30363d;
    padding: 12px 24px;
  }

  nav { display: flex; align-items: baseline; gap: 12px; }

  .logo {
    font-size: 20px; font-weight: 700;
    color: #f0f6fc; letter-spacing: -0.5px;
  }
  .logo:hover { text-decoration: none; }

  .subtitle { font-size: 13px; color: #8b949e; }

  main {
    flex: 1; padding: 24px;
    max-width: 1400px; margin: 0 auto;
    width: 100%; box-sizing: border-box;
  }
</style>
