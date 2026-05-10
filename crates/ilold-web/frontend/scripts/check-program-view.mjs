#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import assert from 'node:assert/strict';

const here = dirname(fileURLToPath(import.meta.url));
const snapshotPath = resolve(
  here,
  '../../../ilold-solana-core/tests/snapshots/staking_view.json',
);

const raw = readFileSync(snapshotPath, 'utf8');
const view = JSON.parse(raw);

assert.equal(typeof view.name, 'string', 'name is string');
assert.equal(typeof view.program_id, 'string', 'program_id is string');
assert.ok(Array.isArray(view.instructions), 'instructions is array');
assert.ok(view.instructions.length > 0, 'instructions non-empty');
assert.ok(Array.isArray(view.accounts), 'accounts is array');

const ix0 = view.instructions[0];
assert.equal(typeof ix0.name, 'string', 'ix.name is string');
assert.equal(typeof ix0.discriminator_hex, 'string', 'ix.discriminator_hex is string');
assert.ok(Array.isArray(ix0.args), 'ix.args is array');
assert.ok(Array.isArray(ix0.accounts), 'ix.accounts is array');
for (const a of ix0.args) {
  assert.equal(typeof a.name, 'string', 'arg.name is string');
  assert.equal(typeof a.ty, 'string', 'arg.ty is string');
}
for (const acc of ix0.accounts) {
  assert.equal(typeof acc.name, 'string', 'ix-account.name is string');
  assert.equal(typeof acc.path, 'string', 'ix-account.path is string');
  assert.ok(
    ['program', 'system', 'sysvar', 'pda', 'other'].includes(acc.kind),
    `ix-account.kind unknown: ${acc.kind}`,
  );
  assert.equal(typeof acc.signer, 'boolean', 'ix-account.signer is boolean');
  assert.equal(typeof acc.writable, 'boolean', 'ix-account.writable is boolean');
}

const pool = view.accounts.find((a) => a.name === 'Pool');
assert.ok(pool, 'Pool account-type present');
assert.ok(Array.isArray(pool.fields), 'Pool.fields is array');
assert.ok(pool.fields.length >= 4, `Pool.fields >= 4 (got ${pool.fields.length})`);
for (const f of pool.fields) {
  assert.equal(typeof f.name, 'string', 'field.name is string');
  assert.equal(typeof f.ty, 'string', 'field.ty is string');
}

if (view.state_coupling) {
  for (const pair of view.state_coupling) {
    assert.equal(typeof pair.a, 'string');
    assert.equal(typeof pair.b, 'string');
    assert.ok(Array.isArray(pair.shared_writable));
  }
}
if (view.admin_gated) {
  assert.ok(Array.isArray(view.admin_gated), 'admin_gated is array');
}
if (view.system_accounts) {
  assert.ok(Array.isArray(view.system_accounts), 'system_accounts is array');
}

console.log(`ok: ProgramView snapshot matches TS contract (${view.instructions.length} ixs, ${view.accounts.length} account-types)`);
