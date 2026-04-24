// Shared Command type used by the Cmd+K palette. Consumers build arrays of
// these at runtime (usually in the route that owns the relevant handlers)
// and register them with `$lib/stores/palette.svelte`.

export type CommandCategory =
  | 'Action'
  | 'Mode'
  | 'Scenario'
  | 'Function'
  | 'Contract'
  | 'Path';

export interface Command {
  id: string;
  label: string;
  /** Secondary line rendered in a dimmer tone under the label. */
  detail?: string;
  category: CommandCategory;
  /** Short glyph / emoji shown before the label. Decorative only. */
  icon?: string;
  /** Extra search tokens so a `Center canvas` command also matches "fit". */
  keywords?: string[];
  /** Invoked when the user confirms the row. May be async. */
  run: () => void | Promise<void>;
}

export const CATEGORY_ORDER: readonly CommandCategory[] = [
  'Action',
  'Mode',
  'Scenario',
  'Function',
  'Contract',
  'Path',
];
