import { writable } from 'svelte/store';

export const searchOpen = writable(false);
export const searchContext = writable<string | null>(null);

export function toggleSearch() {
  searchOpen.update(v => !v);
}

export function setSearchContext(contractName: string | null) {
  searchContext.set(contractName);
}
