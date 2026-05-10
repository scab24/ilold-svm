import { getUserLabels as fetchUserLabels } from '$lib/api/rest';

let scenario = $state<string>('');
let labels = $state<Record<string, string>>({});

export function getUserLabelsScenario(): string {
  return scenario;
}

export function getUserLabelsMap(): Record<string, string> {
  return labels;
}

export function labelForPubkey(pubkey: string): string | null {
  return labels[pubkey] ?? null;
}

export function clearUserLabels(): void {
  scenario = '';
  labels = {};
}

export async function loadUserLabels(scenarioName: string): Promise<void> {
  try {
    const map = await fetchUserLabels(scenarioName);
    scenario = scenarioName;
    labels = { ...map };
  } catch (err) {
    console.warn('userLabels loadUserLabels failed:', err);
    clearUserLabels();
    scenario = scenarioName;
  }
}
