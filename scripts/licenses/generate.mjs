#!/usr/bin/env node
// Regenerates THIRD_PARTY_LICENSES.md: every third-party crate built into the released `mote`
// binary, with its version and license, and the full license texts grouped by license.
//
// Usage (from anywhere in the repository; Node 18+, standard library only):
//   node scripts/licenses/generate.mjs                  write THIRD_PARTY_LICENSES.md
//   node scripts/licenses/generate.mjs --check          exit 1 if the file is missing or out of date
//   node scripts/licenses/generate.mjs --offline        pass --offline to cargo
//   node scripts/licenses/generate.mjs --allow-missing  write the file even when a license text is
//                                                       missing (a visible placeholder takes its
//                                                       place; never for a release)
//   node scripts/licenses/generate.mjs --output FILE    write somewhere else
//
// What is covered: the crates reachable from `mote-cli` through normal and build dependencies
// (procedural macros and build scripts included; dev-dependencies are test-only and left out),
// with all features, on every target that .github/workflows/release.yml builds. Each target is
// resolved with `cargo metadata --format-version 1 --locked --all-features --filter-platform`.
//
// License texts are read from each crate's package in the cargo registry: the license files at
// its root (LICENSE*, LICENCE*, COPYING*, UNLICENSE*, the manifest's `license-file`) and license
// files of code vendored deeper inside it (C sources, Unicode tables). When a crate offers a choice
// (`MIT OR Apache-2.0`), the first alternative, in the preference order below, whose licenses are
// all allowed by deny.toml and all have a text in the package is used, and only its texts are
// reproduced. A crate whose package ships no usable text needs an entry in supplements.json that
// points at a verbatim copy of the upstream text under scripts/licenses/texts/ (`file`, or `files`
// when the crate's code is under several copyright holders' texts of that license, all reproduced;
// `notice` adds a notice the license asks to pass on); without one the script fails. A crate
// whose license is not allowed by deny.toml always fails.

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..', '..');
const ROOT_PACKAGE = 'mote-cli';
const DEFAULT_OUTPUT = join(ROOT, 'THIRD_PARTY_LICENSES.md');
// Used when .github/workflows/release.yml can't be read; keep in sync with its build matrix.
const FALLBACK_TARGETS = [
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'x86_64-unknown-linux-musl',
  'aarch64-unknown-linux-musl',
  'aarch64-apple-darwin',
  'x86_64-apple-darwin',
  'x86_64-pc-windows-msvc',
  'aarch64-pc-windows-msvc',
];
// Which license of a choice is used: earlier is preferred. MIT and the BSD family carry the
// authors' copyright line in the text itself, so they give the most useful attribution.
const PREFERENCE = [
  'MIT',
  'MIT-0',
  'BSD-3-Clause',
  'BSD-2-Clause',
  'ISC',
  'Zlib',
  'BSL-1.0',
  'Unicode-3.0',
  'Unicode-DFS-2016',
  'Apache-2.0',
  'Apache-2.0 WITH LLVM-exception',
  'CC0-1.0',
  'MPL-2.0',
];
const LICENSE_FILE = /^(licen[cs]e|copying|unlicense)/i;
const SKIP_DIRS = new Set(['test', 'tests', 'testdata', 'example', 'examples', 'bench', 'benches', 'fuzz', 'doc', 'docs', 'target']);
const NESTED_DEPTH = 5;

// ---------------------------------------------------------------------------------------------
// Arguments

const args = process.argv.slice(2);
const opts = { check: false, offline: false, allowMissing: false, output: DEFAULT_OUTPUT };
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--check') opts.check = true;
  else if (a === '--offline') opts.offline = true;
  else if (a === '--allow-missing') opts.allowMissing = true;
  else if (a === '--output') opts.output = resolve(args[++i] ?? fail('--output needs a file'));
  else if (a === '-h' || a === '--help') {
    process.stdout.write(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(1, 13).map((l) => l.replace(/^\/\/ ?/, '')).join('\n') + '\n');
    process.exit(0);
  } else fail(`unknown argument ${a} (see --help)`);
}

function fail(message) {
  process.stderr.write(`generate.mjs: ${message}\n`);
  process.exit(1);
}

// ---------------------------------------------------------------------------------------------
// Inputs: release targets, the license allowlist, supplements

function releaseTargets() {
  const file = join(ROOT, '.github', 'workflows', 'release.yml');
  if (!existsSync(file)) return FALLBACK_TARGETS;
  const targets = [...readFileSync(file, 'utf8').matchAll(/\{\s*target:\s*([A-Za-z0-9_.-]+)/g)].map((m) => m[1]);
  if (targets.length === 0) fail(`no "{ target: … }" entries found in ${relative(ROOT, file)}`);
  return [...new Set(targets)];
}

function allowedLicenses() {
  const file = join(ROOT, 'deny.toml');
  if (!existsSync(file)) fail('deny.toml not found: it holds the license allowlist');
  const text = readFileSync(file, 'utf8');
  const section = text.split(/^\[licenses\]\s*$/m)[1]?.split(/^\[/m)[0];
  const list = section?.match(/^allow\s*=\s*\[([\s\S]*?)\]/m)?.[1];
  if (!list) fail('deny.toml: no `allow = [...]` in [licenses]');
  const ids = [...list.replace(/#.*$/gm, '').matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  if (ids.length === 0) fail('deny.toml: the license allowlist is empty');
  return new Set(ids);
}

function loadSupplements() {
  const file = join(HERE, 'supplements.json');
  if (!existsSync(file)) return [];
  const data = JSON.parse(readFileSync(file, 'utf8'));
  for (const e of data.entries ?? []) {
    const files = e.files ?? (e.file ? [e.file] : []);
    if (!Array.isArray(e.crates) || !e.version || !e.license || files.length === 0 || !e.source) {
      fail(`supplements.json: every entry needs crates, version, license, file (or files) and source: ${JSON.stringify(e)}`);
    }
    e.files = files;
  }
  return data.entries ?? [];
}

// ---------------------------------------------------------------------------------------------
// The dependency graph

function cargoMetadata(target) {
  const cargoArgs = ['metadata', '--format-version', '1', '--locked', '--all-features', '--filter-platform', target];
  if (opts.offline) cargoArgs.push('--offline');
  try {
    return JSON.parse(execFileSync('cargo', cargoArgs, { cwd: ROOT, encoding: 'utf8', maxBuffer: 512 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }));
  } catch (e) {
    fail(`cargo ${cargoArgs.join(' ')} failed:\n${e.stderr || e.message}`);
  }
}

function collectPackages(targets) {
  const packages = new Map(); // id -> package
  for (const target of targets) {
    const meta = cargoMetadata(target);
    const byId = new Map(meta.packages.map((p) => [p.id, p]));
    const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
    const root = meta.packages.find((p) => p.name === ROOT_PACKAGE && p.source == null);
    if (!root) fail(`package ${ROOT_PACKAGE} not found in the workspace`);
    const seen = new Set();
    const stack = [root.id];
    while (stack.length) {
      const id = stack.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      for (const dep of nodes.get(id)?.deps ?? []) {
        if (dep.dep_kinds.some((k) => k.kind === null || k.kind === 'build')) stack.push(dep.pkg);
      }
    }
    for (const id of seen) {
      const p = byId.get(id);
      if (p.source != null) packages.set(id, p); // workspace crates are ours
    }
  }
  return [...packages.values()].sort((a, b) => a.name.localeCompare(b.name) || compareVersions(a.version, b.version));
}

function compareVersions(a, b) {
  const pa = a.split(/[.+-]/).map((x) => (/^\d+$/.test(x) ? Number(x) : x));
  const pb = b.split(/[.+-]/).map((x) => (/^\d+$/.test(x) ? Number(x) : x));
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if (pa[i] === pb[i]) continue;
    if (pa[i] === undefined) return -1;
    if (pb[i] === undefined) return 1;
    return typeof pa[i] === 'number' && typeof pb[i] === 'number' ? pa[i] - pb[i] : String(pa[i]).localeCompare(String(pb[i]));
  }
  return 0;
}

// ---------------------------------------------------------------------------------------------
// License expressions and texts

/** `(MIT OR Apache-2.0) AND Unicode-3.0` -> [["MIT","Unicode-3.0"],["Apache-2.0","Unicode-3.0"]] */
function alternatives(expression) {
  const tokens = expression.replace(/\//g, ' OR ').match(/\(|\)|[^\s()]+/g) ?? [];
  let pos = 0;
  const peek = () => tokens[pos];
  const next = () => tokens[pos++];
  function parseOr() {
    let result = parseAnd();
    while (peek() === 'OR') {
      next();
      result = result.concat(parseAnd());
    }
    return result;
  }
  function parseAnd() {
    let result = parseAtom();
    while (peek() === 'AND') {
      next();
      const right = parseAtom();
      result = result.flatMap((l) => right.map((r) => [...l, ...r]));
    }
    return result;
  }
  function parseAtom() {
    const t = next();
    if (t === '(') {
      const inner = parseOr();
      if (next() !== ')') throw new Error(`unbalanced parentheses in "${expression}"`);
      return inner;
    }
    if (t === undefined || t === ')' || t === 'AND' || t === 'OR' || t === 'WITH') throw new Error(`cannot parse "${expression}"`);
    if (peek() === 'WITH') {
      next();
      return [[`${t} WITH ${next()}`]];
    }
    return [[t]];
  }
  const result = parseOr();
  if (pos !== tokens.length) throw new Error(`cannot parse "${expression}"`);
  return result;
}

function normalizeText(text) {
  return text.replace(/^﻿/, '').replace(/\r\n?/g, '\n').split('\n').map((l) => l.replace(/\s+$/, '')).join('\n').replace(/^\n+|\n+$/g, '');
}

/** The SPDX ids a license file's text matches (most files match one). */
function classify(text) {
  const t = text.toLowerCase().replace(/\s+/g, ' ');
  const ids = [];
  if (t.includes('apache license') && t.includes('version 2.0')) {
    ids.push(t.includes('llvm exception') ? 'Apache-2.0 WITH LLVM-exception' : 'Apache-2.0');
  }
  if (t.includes('boost software license')) ids.push('BSL-1.0');
  if (t.includes('unicode license v3')) ids.push('Unicode-3.0');
  else if (t.includes('unicode, inc. license agreement - data files and software')) ids.push('Unicode-DFS-2016');
  else if (t.includes('copyright and permission notice (icu 58 and later)')) ids.push('ICU');
  else if (t.includes('permission is hereby granted, free of charge, to any person obtaining a copy') && t.includes('the above copyright notice and this permission notice shall be included')) {
    ids.push('MIT');
  } else if (t.includes('permission is hereby granted, free of charge, to any person obtaining a copy')) ids.push('MIT-0');
  if (t.includes('redistribution and use in source and binary forms')) {
    ids.push(t.includes('neither the name') || t.includes('may be used to endorse') ? 'BSD-3-Clause' : 'BSD-2-Clause');
  }
  if (t.includes('permission to use, copy, modify, and/or distribute this software for any purpose')) ids.push('ISC');
  if (t.includes("provided 'as-is'") && t.includes('altered source versions must be plainly marked')) ids.push('Zlib');
  if (t.includes('mozilla public license version 2.0')) ids.push('MPL-2.0');
  if (t.includes('cc0 1.0 universal')) ids.push('CC0-1.0');
  if (t.includes('free and unencumbered software released into the public domain')) ids.push('Unlicense');
  if (t.includes('gnu general public license')) ids.push('GPL');
  return ids;
}

function findLicenseFiles(dir, depth, rel = '') {
  const found = [];
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const e of entries) {
    const r = rel ? `${rel}/${e.name}` : e.name;
    if (e.isDirectory()) {
      if (depth > 0 && !e.name.startsWith('.') && !SKIP_DIRS.has(e.name.toLowerCase())) found.push(...findLicenseFiles(join(dir, e.name), depth - 1, r));
    } else if (LICENSE_FILE.test(e.name) && statSync(join(dir, e.name)).size > 0) {
      found.push(r);
    }
  }
  return found.sort();
}

function rank(id) {
  const i = PREFERENCE.indexOf(id);
  return i < 0 ? PREFERENCE.length : i;
}

function analyzePackage(pkg, allowed, supplements) {
  const dir = dirname(pkg.manifest_path);
  const expression = pkg.license ?? (pkg.license_file ? 'LicenseRef-file' : '');
  const problems = [];
  if (!expression) return { problems: [`${pkg.name} ${pkg.version}: no license declared in its manifest`] };

  let alts;
  try {
    alts = alternatives(expression).map((ids) => [...new Set(ids)]);
  } catch (e) {
    return { problems: [`${pkg.name} ${pkg.version}: ${e.message}`] };
  }
  alts.sort((a, b) => Math.max(...a.map(rank)) - Math.max(...b.map(rank)) || a.length - b.length);
  const permitted = alts.filter((ids) => ids.every((id) => allowed.has(id)));
  if (permitted.length === 0) {
    return { problems: [`${pkg.name} ${pkg.version}: license "${expression}" is not allowed by deny.toml`] };
  }

  // Texts at the package root (and the manifest's license-file), by the license they match.
  const rootFiles = findLicenseFiles(dir, 0);
  if (pkg.license_file) {
    const lf = relative(dir, resolve(dir, pkg.license_file)).split(sep).join('/');
    if (!rootFiles.includes(lf) && existsSync(join(dir, lf))) rootFiles.push(lf);
  }
  const texts = new Map(); // id -> [{ file, text }]
  for (const file of rootFiles) {
    const text = normalizeText(readFileSync(join(dir, file), 'utf8'));
    for (const id of classify(text)) {
      if (!texts.has(id)) texts.set(id, []);
      texts.get(id).push({ file, text });
    }
  }
  // A supplement's texts are all reproduced (and its notice, if any), not only the first.
  const supplied = new Map(); // id -> [{ file, text, supplement, notice }]
  for (const s of supplements) {
    if (!s.crates.includes(pkg.name) || s.version !== pkg.version) continue;
    const entries = [];
    for (const [name, notice] of [...s.files.map((f) => [f, false]), ...(s.notice ? [[s.notice, true]] : [])]) {
      const file = join(HERE, name);
      if (!existsSync(file)) {
        problems.push(`${pkg.name} ${pkg.version}: its package ships no usable ${s.license} ${notice ? 'notice' : 'text'}; save the upstream text (${s.source}) verbatim as scripts/licenses/${name}`);
        continue;
      }
      const text = normalizeText(readFileSync(file, 'utf8'));
      if (!notice && !classify(text).includes(s.license)) problems.push(`scripts/licenses/${name} does not look like a ${s.license} text`);
      entries.push({ file: `scripts/licenses/${name}`, text, supplement: s.source, notice });
    }
    if (entries.length === s.files.length + (s.notice ? 1 : 0)) {
      if (!texts.has(s.license)) texts.set(s.license, []);
      texts.get(s.license).push(entries[0]);
      supplied.set(s.license, [...(supplied.get(s.license) ?? []), ...entries]);
    }
  }

  const chosen = permitted.find((ids) => ids.every((id) => texts.has(id)));
  if (!chosen) {
    if (problems.length === 0) {
      const files = rootFiles.length ? rootFiles.join(', ') : 'no license file';
      problems.push(`${pkg.name} ${pkg.version} ("${expression}"): no license text found for any allowed choice (${files}); add an entry to scripts/licenses/supplements.json with the upstream text`);
    }
    return { expression, problems, chosen: permitted[0], used: [], nested: [] };
  }
  const used = chosen.flatMap((id) => (supplied.get(id) ?? [texts.get(id)[0]]).map((t) => ({ id, ...t })));

  // License files of code vendored deeper inside the package (C sources, data tables).
  const nested = findLicenseFiles(dir, NESTED_DEPTH)
    .filter((f) => f.includes('/'))
    .map((file) => {
      const text = normalizeText(readFileSync(join(dir, file), 'utf8'));
      // Classify by the head of the file: some (ICU's) append the licenses of their own parts.
      return { id: classify(text.slice(0, 2000))[0] ?? classify(text)[0] ?? 'Other', file, text };
    });
  return { expression, problems: [], chosen, used, nested };
}

// ---------------------------------------------------------------------------------------------
// Output

function fence(text) {
  const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  return '`'.repeat(Math.max(3, longest + 1));
}

function anchor(id) {
  return id.toLowerCase().replace(/[^a-z0-9 -]/g, '').replace(/ /g, '-');
}

function render(packages, results, targets) {
  const L = [];
  L.push('# Third-party licenses');
  L.push('');
  L.push('The `mote` release binaries contain code from the open-source crates listed below. Their');
  L.push('licenses ask for their copyright and license notices to be passed on with the binaries; the full');
  L.push('texts are in [License texts](#license-texts). Mote itself is licensed under MIT OR Apache-2.0');
  L.push('([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).');
  L.push('');
  L.push('This file is generated by `node scripts/licenses/generate.mjs` from `Cargo.lock`; do not edit it');
  L.push('by hand. It covers every crate built into the `mote` binary (all features) on the release targets');
  L.push(`(${targets.map((t) => `\`${t}\``).join(', ')}): normal and build dependencies, procedural macros`);
  L.push('included; test-only dependencies are not part of the binary and are left out. Where a crate offers');
  L.push('a choice of licenses, Mote uses it under the one in the *Used under* column, and that is the text');
  L.push('reproduced below.');
  L.push('');
  const missing = packages.filter((p) => results.get(p.id).used.length === 0);
  if (missing.length) {
    L.push(`> **Incomplete:** ${missing.length} crate(s) below have no license text yet (marked *missing*).`);
    L.push('> This file must not ship until `node scripts/licenses/generate.mjs` succeeds without');
    L.push('> `--allow-missing`.');
    L.push('');
  }
  L.push('## Crates');
  L.push('');
  L.push(`${packages.length} crates.`);
  L.push('');
  L.push('| Crate | Version | License (as declared) | Used under | Source |');
  L.push('|---|---|---|---|---|');
  for (const p of packages) {
    const r = results.get(p.id);
    const used = r.used.length ? r.chosen.map((id) => `[${id}](#${anchor(id)})`).join(' AND ') : '*missing*';
    const url = p.repository || p.homepage || `https://crates.io/crates/${p.name}`;
    L.push(`| ${p.name} | ${p.version} | ${(p.license ?? '').replace(/\|/g, '\\|')} | ${used} | ${url} |`);
  }
  L.push('');

  // Group distinct texts by license id.
  const groups = new Map(); // id -> Map(normalized text -> { text, users: [] })
  const add = (id, text, user) => {
    if (!groups.has(id)) groups.set(id, new Map());
    const key = text.replace(/\s+/g, ' ').trim();
    const g = groups.get(id);
    if (!g.has(key)) g.set(key, { text, users: [] });
    g.get(key).users.push(user);
  };
  for (const p of packages) {
    const r = results.get(p.id);
    for (const u of r.used) {
      const where = u.supplement ? `upstream ${u.notice ? 'notice' : 'text'}, ${u.supplement}` : u.file;
      add(u.id, u.text, `${p.name} ${p.version} (${where})`);
    }
    for (const n of r.nested) add(n.id, n.text, `${p.name} ${p.version} (${n.file}, code vendored in the crate)`);
  }
  L.push('## License texts');
  L.push('');
  const ids = [...groups.keys()].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
  for (const id of ids) {
    const entries = [...groups.get(id).values()];
    L.push(`### ${id}`);
    L.push('');
    entries.forEach((e, i) => {
      if (entries.length > 1) {
        L.push(`#### ${id}, text ${i + 1} of ${entries.length}`);
        L.push('');
      }
      L.push(`Used by: ${e.users.join('; ')}.`);
      L.push('');
      const f = fence(e.text);
      L.push(f + 'text');
      L.push(e.text);
      L.push(f);
      L.push('');
    });
  }
  if (missing.length) {
    L.push('### Missing texts');
    L.push('');
    for (const p of missing) {
      const why = results.get(p.id).problems.map((m) => m.replace(`${p.name} ${p.version}: `, ''));
      L.push(`* ${p.name} ${p.version} (${p.license}): ${why.join('; ')}`);
    }
    L.push('');
  }
  return L.join('\n');
}

// ---------------------------------------------------------------------------------------------

const targets = releaseTargets();
const allowed = allowedLicenses();
const supplements = loadSupplements();
const packages = collectPackages(targets);
const results = new Map(packages.map((p) => [p.id, analyzePackage(p, allowed, supplements)]));

const problems = packages.flatMap((p) => results.get(p.id).problems);
const notAllowed = problems.filter((m) => m.includes('not allowed by deny.toml') || m.includes('no license declared') || m.includes('cannot parse') || m.includes('does not look like'));
if (problems.length) {
  process.stderr.write(`generate.mjs: ${problems.length} problem(s):\n${problems.map((m) => `  * ${m}`).join('\n')}\n`);
  if (!opts.allowMissing || notAllowed.length) process.exit(1);
  process.stderr.write('generate.mjs: --allow-missing: writing the file with placeholders; it must not ship like this.\n');
}

const output = render(packages, results, targets) + '\n';
if (opts.check) {
  const current = existsSync(opts.output) ? readFileSync(opts.output, 'utf8').replace(/\r\n/g, '\n') : null;
  if (current !== output) fail(`${relative(ROOT, opts.output)} is out of date; run node scripts/licenses/generate.mjs`);
  process.stdout.write(`${relative(ROOT, opts.output)} is up to date (${packages.length} crates).\n`);
} else {
  writeFileSync(opts.output, output);
  process.stdout.write(`wrote ${relative(ROOT, opts.output)}: ${packages.length} crates, ${new Set(packages.flatMap((p) => results.get(p.id).chosen ?? [])).size} licenses.\n`);
}
