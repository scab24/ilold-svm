import { writable } from 'svelte/store';

export const searchOpen = writable(false);
export const searchContext = writable<string | null>(null);

// Callback for when user selects a search result — canvas listens to this
export const searchNavigate = writable<{ contract: string; func: string; pathId: number } | null>(null);

export function toggleSearch() {
  searchOpen.update(v => !v);
}

export function setSearchContext(contractName: string | null) {
  searchContext.set(contractName);
}
