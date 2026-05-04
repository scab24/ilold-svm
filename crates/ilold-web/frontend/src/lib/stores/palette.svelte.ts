// Command-palette state. Kept in a tiny store so `+layout.svelte` can own
// the <CommandPalette /> instance (single mount, always-live Cmd+K binding)
// while each route publishes its context-specific commands here.

import type { Command } from '$lib/commands/registry';

let _open = $state(false);
let _commands = $state<Command[]>([]);

export function isPaletteOpen(): boolean { return _open; }
export function openPalette() { _open = true; }
export function closePalette() { _open = false; }
export function togglePalette() { _open = !_open; }

export function getPaletteCommands(): Command[] { return _commands; }

/** Replace the full command list. Routes typically call this inside an
 *  `$effect` so the list stays in sync with the reactive state it derives
 *  from (active scenario, loaded contract, etc.). */
export function setPaletteCommands(cmds: Command[]) { _commands = cmds; }

/** Convenience for unmounting routes: clear everything. */
export function clearPaletteCommands() { _commands = []; }
