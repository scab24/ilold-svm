#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import assert from 'node:assert/strict';

const here = dirname(fileURLToPath(import.meta.url));
const snapshotPath = resolve(
  here,
  '../../../ilold-solana-core/tests/snapshots/staking_overlay.json',
);

const raw = readFileSync(snapshotPath, 'utf8');
const overlay = JSON.parse(raw);

assert.equal(typeof overlay.program, 'string', 'program is string');
assert.equal(typeof overlay.scenario, 'string', 'scenario is string');

assert.equal(typeof overlay.calls_per_ix, 'object', 'calls_per_ix is object');
assert.ok(!Array.isArray(overlay.calls_per_ix), 'calls_per_ix is map, not array');
for (const [k, v] of Object.entries(overlay.calls_per_ix)) {
  assert.equal(typeof k, 'string', `calls_per_ix key is string`);
  assert.equal(typeof v, 'number', `calls_per_ix["${k}"] is number`);
}

assert.equal(typeof overlay.failed_per_ix, 'object', 'failed_per_ix is object');
assert.ok(!Array.isArray(overlay.failed_per_ix), 'failed_per_ix is map, not array');
for (const [k, v] of Object.entries(overlay.failed_per_ix)) {
  assert.equal(typeof k, 'string', `failed_per_ix key is string`);
  assert.equal(typeof v, 'number', `failed_per_ix["${k}"] is number`);
}

assert.equal(typeof overlay.cu_stats_per_ix, 'object', 'cu_stats_per_ix is object');
assert.ok(!Array.isArray(overlay.cu_stats_per_ix), 'cu_stats_per_ix is map, not array');
for (const [k, stats] of Object.entries(overlay.cu_stats_per_ix)) {
  assert.equal(typeof stats.min, 'number', `${k}.min is number`);
  assert.equal(typeof stats.max, 'number', `${k}.max is number`);
  assert.equal(typeof stats.avg, 'number', `${k}.avg is number`);
  assert.equal(typeof stats.samples, 'number', `${k}.samples is number`);
}

assert.ok(Array.isArray(overlay.cpi_edges), 'cpi_edges is array');
overlay.cpi_edges.forEach((edge, i) => {
  assert.equal(typeof edge.from_ix, 'string', `cpi_edges[${i}].from_ix is string`);
  assert.equal(typeof edge.to_program, 'string', `cpi_edges[${i}].to_program is string`);
  assert.equal(typeof edge.depth, 'number', `cpi_edges[${i}].depth is number`);
  assert.equal(typeof edge.samples, 'number', `cpi_edges[${i}].samples is number`);
});

console.log(
  `ok: RuntimeOverlay snapshot matches TS contract (program=${overlay.program}, scenario=${overlay.scenario}, ${overlay.cpi_edges.length} cpi edges)`,
);
