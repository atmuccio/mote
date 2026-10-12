// Collections filled but never read.
export function validate(rows) {
  const errors = []; // expect: unread-collection
  for (const row of rows) {
    if (!row.id) errors.push('row without id');
  }
  return true;
}

export function index(items) {
  const byId = new Map(); // expect: unread-collection
  const seen = new Set(); // expect: unread-collection
  const slots = []; // expect: unread-collection
  items.forEach((item, i) => {
    byId.set(item.id, item);
    seen.add(item.id);
    slots[i] = item;
  });
  return items.length;
}

// Compliant.
export function used(rows) {
  const errors = [];
  for (const row of rows) {
    if (!row.id) errors.push('row without id');
  }
  return errors;
}

export function iterated(items) {
  const ids = [];
  items.forEach((i) => ids.push(i.id));
  for (const id of ids) console.log(id);
}

export function passed(items) {
  const out = [];
  out.push(...items);
  send(out);
}

export function lengthRead(items) {
  const out = [];
  out.push(items[0]);
  return out.length;
}

export function chained() {
  const map = new Map();
  map.set('a', 1).set('b', 2);
}

export function returnedFromPush(items) {
  const out = [];
  return out.push(items);
}

export const registry = [];
registry.push('exported');

const moduleCache = new Map();
export function remember(key, value) {
  moduleCache.set(key, value);
}
export function recall(key) {
  return moduleCache.get(key);
}

export function reassigned() {
  let list = [];
  list.push(1);
  list = [];
  return 0;
}

export function unknownMethod() {
  const list = [];
  list.push(1);
  list.customMethod();
}

const exportedLater = [];
exportedLater.push('a');
export { exportedLater };
