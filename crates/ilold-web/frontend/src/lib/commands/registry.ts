// Shared Command type used by the Cmd+K palette. Consumers build arrays of
// these at runtime (usually in the route that owns the relevant handlers)
// and register them with `$lib/stores/palette.svelte`.

export type CommandCategory =
  | 'Recent'
  | 'Suggestion'
  | 'Action'
  | 'Mode'
  | 'Scenario'
  | 'Function'
  | 'Contract'
  | 'Path';

/** Rich metadata for path search rows — the palette uses this to render
 *  coloured match tokens (require / external_call / state_write / event /
 *  assembly) and the terminal pill (Return / Revert). Keeping it here
 *  instead of stuffing coloured HTML into `detail` lets the palette own
 *  the styling and stay XSS-safe. */
export interface PathMatchMeta {
  contract: string;
  terminal: string;
  matches: { field: string; value: string }[];
}

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
  /** When set, the palette renders the row with coloured match pills. */
  pathMeta?: PathMatchMeta;
  /** When set, the palette renders a kbd-style chip on the right of the
   *  row so users can learn the keyboard shortcut for this action. */
  shortcut?: string;
  /** By default the palette closes as soon as a command runs. Suggestions
   *  are the exception — they pre-fill the input so the user can watch
   *  path results stream in, so the palette must stay open. */
  keepOpenOnRun?: boolean;
  /** Invoked when the user confirms the row. May be async. */
  run: () => void | Promise<void>;
}

export const CATEGORY_ORDER: readonly CommandCategory[] = [
  'Recent',
  'Suggestion',
  'Action',
  'Mode',
  'Scenario',
  'Function',
  'Contract',
  'Path',
];
