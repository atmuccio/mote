const a = { x: 1, x: 2 }; // expect: no-dupe-keys
const b = {
  y: 1,
  'y': 2, // expect: no-dupe-keys
  ['y']: 3, // expect: no-dupe-keys
};
const c = { 1: 'a', 1.0: 'b', 0x1: 'c', '1': 'd' }; // expect: no-dupe-keys, no-dupe-keys, no-dupe-keys
const d = { get p() { return 1; }, set p(v) {} };
const e = { get q() { return 1; }, get q() { return 2; } }; // expect: no-dupe-keys
const f = { r: 1, get r() { return 2; } }; // expect: no-dupe-keys
const g = { ...h, s: 1, ...i, s2: 2 };
const k = { [key]: 1, [key]: 2 };
const l = { __proto__: null, a: 1 };
const m = { a, a }; // expect: no-dupe-keys
const n = { m() {}, m() {} }; // expect: no-dupe-keys
const o = { 1e21: 'x', '1e+21': 'y' };
const p = { a: 1, b: { a: 2 } };
const q = { a: 1, A: 2, 'a ': 3 };
class X { a() {} a() {} }
