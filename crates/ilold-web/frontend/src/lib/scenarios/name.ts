// Shared scenario-name validation + prompt helper.
// Mirrors the backend regex enforced in `crates/ilold-core/src/exploration/store.rs`.

export const SCENARIO_NAME_REGEX = /^[a-z][a-z0-9_-]{0,31}$/;

const DEFAULT_LABEL = 'New scenario name (lowercase, a-z 0-9 _ -, max 32):';

/** Prompts for a scenario name and validates it client-side.
 *  Returns the trimmed name on success, or `null` on cancel/invalid input
 *  (the reason is logged to the console). */
export function promptScenarioName(label: string = DEFAULT_LABEL): string | null {
  const raw = prompt(label);
  if (!raw) return null;
  const name = raw.trim();
  if (!SCENARIO_NAME_REGEX.test(name)) {
    console.warn(`Invalid scenario name '${name}'. Must match ${SCENARIO_NAME_REGEX}`);
    return null;
  }
  return name;
}
