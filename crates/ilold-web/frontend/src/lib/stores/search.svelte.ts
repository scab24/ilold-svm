// Navigation channel used by the Cmd+K palette. When the palette runs a
// path-search result, it writes to _searchNavigate; the contract page's
// $effect picks it up and focuses the target function + path. The
// "search open" / toggle state moved to `$lib/stores/palette.svelte` with
// the rest of the command-palette plumbing.

import type { SearchNavigatePayload } from '$lib/api/types';

let _searchContext = $state<string | null>(null);
let _searchNavigate = $state<SearchNavigatePayload | null>(null);

export function getSearchContext(): string | null { return _searchContext; }
export function setSearchContext(v: string | null) { _searchContext = v; }

export function getSearchNavigate(): SearchNavigatePayload | null { return _searchNavigate; }
export function setSearchNavigate(v: SearchNavigatePayload | null) { _searchNavigate = v; }
