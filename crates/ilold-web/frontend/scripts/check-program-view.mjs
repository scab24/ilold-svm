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

const KINDS = new Set(['program', 'system', 'sysvar', 'pda', 'other']);
const SEED_KINDS = new Set(['const', 'arg', 'account']);

function checkSeed(seed, where) {
  assert.equal(typeof seed, 'object', `${where}: seed is object`);
  assert.ok(SEED_KINDS.has(seed.kind), `${where}: seed.kind unknown: ${seed.kind}`);
  if (seed.kind === 'const') {
    assert.equal(typeof seed.value_hex, 'string', `${where}: const.value_hex is string`);
    if (seed.value_utf8 !== undefined) {
      assert.equal(typeof seed.value_utf8, 'string', `${where}: const.value_utf8 is string`);
    }
  } else if (seed.kind === 'arg') {
    assert.equal(typeof seed.name, 'string', `${where}: arg.name is string`);
    assert.equal(typeof seed.ty, 'string', `${where}: arg.ty is string`);
  } else if (seed.kind === 'account') {
    assert.equal(typeof seed.path, 'string', `${where}: account.path is string`);
  }
}

function checkPda(pda, where) {
  assert.equal(typeof pda, 'object', `${where}: pda is object`);
  assert.ok(Array.isArray(pda.seeds), `${where}: pda.seeds is array`);
  pda.seeds.forEach((s, i) => checkSeed(s, `${where}.seeds[${i}]`));
  if (pda.program !== undefined) {
    assert.equal(typeof pda.program, 'string', `${where}: pda.program is string`);
  }
  if (pda.bump_arg !== undefined) {
    assert.equal(typeof pda.bump_arg, 'string', `${where}: pda.bump_arg is string`);
  }
}

let pdaCount = 0;
view.instructions.forEach((ix, ixIdx) => {
  const where = `ix[${ixIdx}] (${ix.name})`;
  assert.equal(typeof ix.name, 'string', `${where}: name is string`);
  assert.equal(typeof ix.discriminator_hex, 'string', `${where}: discriminator_hex is string`);
  assert.ok(Array.isArray(ix.args), `${where}: args is array`);
  assert.ok(Array.isArray(ix.accounts), `${where}: accounts is array`);
  for (const a of ix.args) {
    assert.equal(typeof a.name, 'string', `${where}: arg.name is string`);
    assert.equal(typeof a.ty, 'string', `${where}: arg.ty is string`);
  }
  ix.accounts.forEach((acc, accIdx) => {
    const accWhere = `${where}.accounts[${accIdx}] (${acc.name})`;
    assert.equal(typeof acc.name, 'string', `${accWhere}: name is string`);
    assert.equal(typeof acc.path, 'string', `${accWhere}: path is string`);
    assert.ok(KINDS.has(acc.kind), `${accWhere}: kind unknown: ${acc.kind}`);
    assert.equal(typeof acc.signer, 'boolean', `${accWhere}: signer is boolean`);
    assert.equal(typeof acc.writable, 'boolean', `${accWhere}: writable is boolean`);
    assert.equal(typeof acc.optional, 'boolean', `${accWhere}: optional is boolean`);
    if (acc.pda !== undefined) {
      checkPda(acc.pda, `${accWhere}.pda`);
      pdaCount += 1;
    }
  });
});

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

console.log(
  `ok: ProgramView snapshot matches TS contract (${view.instructions.length} ixs, ${view.accounts.length} account-types, ${pdaCount} pda accounts)`,
);
