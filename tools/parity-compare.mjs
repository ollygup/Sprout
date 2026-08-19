#!/usr/bin/env node
// Parity compare: legacy Setup.bat log vs Sprout run results (ticket 10).
// Usage: node tools/parity-compare.mjs --legacy <setup.log> [--db <sprout.db>]
//   default db: %LOCALAPPDATA%\Sprout\sprout.db
// Exit: 0 = parity, 1 = mismatch, 2 = unusable inputs.

import { DatabaseSync } from 'node:sqlite';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

function usage() {
  console.error(
    'Usage: node tools/parity-compare.mjs --legacy <setup.log> [--db <sprout.db>]'
  );
  process.exit(2);
}

const args = process.argv.slice(2);
const opt = { legacy: null, db: null };
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--legacy') opt.legacy = args[++i];
  else if (args[i] === '--db') opt.db = args[++i];
}
if (!opt.legacy) usage();
if (!opt.db) {
  opt.db = path.join(
    process.env.LOCALAPPDATA || '',
    'Sprout',
    'sprout.db'
  );
}

// --- legacy -----------------------------------------------------------------

let logText;
try {
  logText = readFileSync(opt.legacy, 'utf8');
} catch (e) {
  console.error(`cannot read legacy log: ${e.message}`);
  process.exit(2);
}

const runs = logText.split(/^===== Setup run:.*=====$/gm);
const lastRun = runs[runs.length - 1] || '';
const legacy = new Map(); // id -> {name, ok: boolean, detail}
let currentId = null;
for (const line of lastRun.split(/\r?\n/)) {
  const block = line.match(/^==== (.+) \(([^)]+)\) ====$/);
  if (block) {
    currentId = block[2].trim();
    legacy.set(currentId, { name: block[1].trim(), ok: null, detail: '' });
    continue;
  }
  const result = line.match(/^(OK|FAILED): (.+)$/);
  if (result && currentId && legacy.has(currentId)) {
    legacy.get(currentId).ok = result[1] === 'OK';
    legacy.get(currentId).detail = result[2];
  }
}
for (const [id, r] of legacy) {
  if (r.ok === null) {
    console.error(`legacy log: no outcome recorded for '${id}'`);
    process.exit(2);
  }
}
if (legacy.size === 0) {
  console.error('legacy log: no per-requirement blocks found');
  process.exit(2);
}

// --- sprout -----------------------------------------------------------------

let db;
try {
  db = new DatabaseSync(opt.db, { readOnly: true });
} catch (e) {
  console.error(`cannot open sprout db: ${e.message}`);
  process.exit(2);
}
const runRow = db.prepare('SELECT id, outcome, started_at FROM runs ORDER BY started_at DESC LIMIT 1').get();
if (!runRow) {
  console.error('sprout db: no runs found — run Sprout once on this machine first');
  process.exit(2);
}
const sprout = new Map();
for (const r of db.prepare('SELECT product_id, status, detail FROM run_results WHERE run_id = ?').all(runRow.id)) {
  sprout.set(r.product_id, { status: r.status, detail: r.detail });
}

// --- compare ----------------------------------------------------------------

const OK_STATUSES = new Set([
  'installed',
  'upgraded',
  'already_ok',
  'satisfied_by_newer',
  'skipped_unmanaged',
]);
const FAIL_STATUSES = new Set(['failed', 'timed_out']);

function sproutClass(s) {
  if (OK_STATUSES.has(s)) return 'ok';
  if (FAIL_STATUSES.has(s)) return 'failed';
  return null;
}

const ids = new Set([...legacy.keys(), ...sprout.keys()]);
let mismatches = 0;
console.log(`${'id'.padEnd(24)} ${'legacy'.padEnd(9)} ${'sprout'.padEnd(20)} result`);
console.log('-'.repeat(72));
for (const id of [...ids].sort()) {
  const l = legacy.get(id);
  const s = sprout.get(id);
  let result;
  if (!l) {
    result = 'missing in legacy';
    mismatches++;
  } else if (!s) {
    result = 'missing in sprout';
    mismatches++;
  } else if (sproutClass(s.status) === null) {
    result = `unknown sprout status '${s.status}'`;
    mismatches++;
  } else if (l.ok === (sproutClass(s.status) === 'ok')) {
    result = 'MATCH';
  } else {
    result = 'MISMATCH';
    mismatches++;
  }
  console.log(
    `${id.padEnd(24)} ${(l ? (l.ok ? 'OK' : 'FAILED') : '-').padEnd(9)} ${(s ? s.status : '-').padEnd(20)} ${result}`
  );
}
console.log('-'.repeat(72));
console.log(
  `legacy run: ${[...legacy.keys()].length} requirements | sprout run: ${sprout.size} results`
);
if (mismatches > 0) {
  console.log(`VERDICT: FAIL (${mismatches} mismatch(es))`);
  process.exit(1);
}
console.log('VERDICT: PASS — per-Requirement outcomes are equivalent');
process.exit(0);
