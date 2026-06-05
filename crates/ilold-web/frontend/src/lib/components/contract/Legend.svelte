<script lang="ts">
  let {
    mode,
    kind = 'solana',
  }: {
    mode: 'cfg' | 'sequences' | 'session';
    kind?: 'solidity' | 'solana';
  } = $props();
</script>

<div
  class="fixed bottom-3 left-4 flex gap-3 text-[11px] text-text-muted px-3.5 py-2 z-10"
  style="
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--color-border) 50%, transparent);
    background: linear-gradient(180deg, rgba(30, 30, 40, 0.85) 0%, rgba(24, 24, 30, 0.9) 100%);
    backdrop-filter: blur(16px) saturate(1.8);
    -webkit-backdrop-filter: blur(16px) saturate(1.8);
    box-shadow:
      0 4px 16px -4px rgba(0, 0, 0, 0.3),
      0 0 0 1px rgba(91, 155, 213, 0.04);
  "
>
  {#if kind === 'solana' && mode === 'cfg'}
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-accent); box-shadow: 0 0 6px color-mix(in srgb, var(--color-accent) 40%, transparent);"></span>Instruction</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; border: 1px solid var(--color-success);"></span>Account type</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-warning); box-shadow: 0 0 6px color-mix(in srgb, var(--color-warning) 40%, transparent);"></span>PDA / signer</span>
    <span class="text-text-dim">Click an instruction → add to canvas</span>
  {:else if kind === 'solana' && mode === 'session'}
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-accent); box-shadow: 0 0 6px color-mix(in srgb, var(--color-accent) 40%, transparent);"></span>Trace step</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-danger); box-shadow: 0 0 6px color-mix(in srgb, var(--color-danger) 40%, transparent);"></span>Failed</span>
    <span class="text-text-dim">Right-click → fork scenario · ⌘K → Execute</span>
  {:else if kind === 'solana'}
    <span class="text-text-dim">Sequences view is not implemented for Solana yet — switch to Program or Session</span>
  {:else if mode === 'cfg'}
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-accent); box-shadow: 0 0 6px color-mix(in srgb, var(--color-accent) 40%, transparent);"></span>Function</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-accent-dark); box-shadow: 0 0 6px color-mix(in srgb, var(--color-accent-dark) 40%, transparent);"></span>Entry block</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-success); box-shadow: 0 0 6px color-mix(in srgb, var(--color-success) 40%, transparent);"></span>Return</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-danger); box-shadow: 0 0 6px color-mix(in srgb, var(--color-danger) 40%, transparent);"></span>Revert</span>
    <span class="text-text-dim">Click → expand CFG</span>
  {:else}
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-accent); box-shadow: 0 0 6px color-mix(in srgb, var(--color-accent) 40%, transparent);"></span>State-changing</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; border: 1px solid var(--color-text-muted); background: transparent;"></span>Read-only</span>
    <span><span class="inline-block w-2 h-2 align-middle mr-1" style="border-radius: 4px; background: var(--color-warning); box-shadow: 0 0 6px color-mix(in srgb, var(--color-warning) 40%, transparent);"></span>Conditions affected</span>
    <span class="text-text-dim">Click → expand · Shift+click → add branch</span>
  {/if}
</div>
