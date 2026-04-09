import type { SearchNavigatePayload } from '$lib/api/types';

let _searchOpen = $state(false);
let _searchContext = $state<string | null>(null);
let _searchNavigate = $state<SearchNavigatePayload | null>(null);

export function getSearchOpen(): boolean { return _searchOpen; }
export function setSearchOpen(v: boolean) { _searchOpen = v; }
export function toggleSearch() { _searchOpen = !_searchOpen; }

export function getSearchContext(): string | null { return _searchContext; }
export function setSearchContext(v: string | null) { _searchContext = v; }

export function getSearchNavigate(): SearchNavigatePayload | null { return _searchNavigate; }
export function setSearchNavigate(v: SearchNavigatePayload | null) { _searchNavigate = v; }
