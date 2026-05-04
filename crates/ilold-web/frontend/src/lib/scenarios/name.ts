// Shared scenario-name validation + prompt helper.
// Mirrors the backend regex enforced in `crates/ilold-core/src/exploration/store.rs`.

export const SCENARIO_NAME_REGEX = /^[a-z][a-z0-9_-]{0,31}$/;

const DEFAULT_LABEL = 'New scenario name (lowercase, a-z 0-9 _ -, max 32):';

/** Prompts for a scenario name and validates it client-side. Returns the
 *  trimmed name, or `null` on cancel. On invalid input the user sees an
 *  alert with the constraints — silent failures left users wondering why
 *  the fork didn't happen. */
export function promptScenarioName(label: string = DEFAULT_LABEL): string | null {
  const raw = prompt(label);
  if (raw === null) return null;
  const name = raw.trim();
  if (!SCENARIO_NAME_REGEX.test(name)) {
    alert(`Invalid scenario name "${name}".\n\nMust be lowercase a-z, numbers, _ or -. Start with a letter. Max 32 chars.`);
    return null;
  }
  return name;
}
