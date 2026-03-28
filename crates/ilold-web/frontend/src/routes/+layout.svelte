<script lang="ts">
  let { children } = $props();
</script>

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

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
    background: #0d1117;
    color: #c9d1d9;
  }

  :global(a) {
    color: #58a6ff;
    text-decoration: none;
  }

  :global(a:hover) {
    text-decoration: underline;
  }

  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
  }

  header {
    background: #161b22;
    border-bottom: 1px solid #30363d;
    padding: 12px 24px;
  }

  nav {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .logo {
    font-size: 20px;
    font-weight: 700;
    color: #f0f6fc;
    letter-spacing: -0.5px;
  }

  .logo:hover {
    text-decoration: none;
  }

  .subtitle {
    font-size: 13px;
    color: #8b949e;
  }

  main {
    flex: 1;
    padding: 24px;
    max-width: 1400px;
    margin: 0 auto;
    width: 100%;
    box-sizing: border-box;
  }
</style>
