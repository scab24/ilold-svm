import type { AccessLevel } from '$lib/api/types';

export function formatAccess(access: AccessLevel): string {
  if (typeof access === 'string') return access;
  if ('Restricted' in access) return `Restricted(${access.Restricted.role ?? 'unknown'})`;
  if ('Special' in access) return `Special(${access.Special.kind ?? 'unknown'})`;
  return 'Unknown';
}
