// Shared dispatch helper for scenario lifecycle actions. Centralizes the
// try/catch + console.warn pattern so call sites read as intent ("delete
// this scenario") rather than boilerplate around postCommand.

import { postCommand, type ScenarioAction } from '$lib/api/session';

/** Post a Scenario command and log any error. `label` is embedded in the
 *  console warning (e.g. "new", "switch", "delete", "fork"). */
export async function dispatchScenarioAction(
  action: ScenarioAction,
  contract: string | undefined,
  label: string,
): Promise<void> {
  try {
    await postCommand({ Scenario: { sub: action } }, contract);
  } catch (e) {
    const reason = e instanceof Error ? e.message : String(e);
    console.warn(`scenario ${label} failed:`, e);
    alert(`Scenario ${label} failed:\n\n${reason}`);
  }
}
