// Collections that are read but never filled.
export function notifyAll(users) {
  const seen = new Set();
  for (const user of users) {
    if (seen.has(user.id)) continue; // expect: empty-collection-access
    notify(user);
  }
}

export function total() {
  const prices = [];
  let sum = 0;
  for (const p of prices) sum += p; // expect: empty-collection-access
  return sum + prices.length;
}

export function lookup(key) {
  const cache = new Map();
  return cache.get(key); // expect: empty-collection-access
}

// Compliant.
export function filled(users) {
  const seen = new Set();
  for (const user of users) {
    if (seen.has(user.id)) continue;
    seen.add(user.id);
  }
}

export function preFilled() {
  const items = [1, 2, 3];
  return items.map((n) => n * 2);
}

export function fromConstructorArg(list) {
  const set = new Set(list);
  return set.has(1);
}

export function escapes() {
  const bucket = [];
  fill(bucket);
  return bucket.length;
}

export function indexWrite(n) {
  const out = [];
  for (let i = 0; i < n; i++) out[i] = i;
  return out.join(',');
}

export function sized() {
  const out = [];
  out.length = 3;
  return out.map((x) => x);
}

export function returned() {
  const empty = [];
  return empty;
}

const shared = [];
export function add(x) {
  shared.push(x);
}
export function read() {
  return shared.slice();
}

const RESERVED_IDS = [];
export function isReserved(id) {
  return RESERVED_IDS.includes(id);
}

export function combine(defaults) {
  const overrides = [];
  return [...defaults, ...overrides];
}

export function commented(x) {
  const extra = [
    // 'legacy',
  ];
  return extra.includes(x);
}
