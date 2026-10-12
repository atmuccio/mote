// Updating a variable from its own value is not a read.

export function countOnly(items) {
  let hits = 0; // expect: no-unused-vars
  for (const item of items) {
    if (item) hits++;
  }
  return items.length;
}

export function compoundOnly(values) {
  let total = 0; // expect: no-unused-vars
  values.forEach((v) => {
    total += v;
  });
}

export function loopCounterOnly(next) {
  for (let i = 0; ; i++) { // expect: no-unused-vars
    if (next()) break;
  }
}

export function sequenceOnly(a) {
  let x = a, y = a; // expect: no-unused-vars, no-unused-vars
  x++, y--;
}

let moduleCounter = 0; // expect: no-unused-vars
export function tick() {
  moduleCounter++;
}

// ---- compliant ----

export function countedAndRead(items) {
  let hits = 0;
  for (const item of items) {
    if (item) hits++;
  }
  return hits;
}

export function valueUsed(list) {
  let index = 0;
  const first = list[index++];
  return first;
}

export function returned(n) {
  let c = n;
  return ++c;
}

export const bump = () => {
  let n = 0;
  return () => n++;
};

export function logicalAssign(v) {
  let cache = null;
  cache ||= v;
}

export function conditionRead(list) {
  let i = 0;
  while (i < list.length) i++;
}

export function selfAssignRead(list) {
  let size = 0;
  size = size + list.length;
  return size;
}

export function readByTemplate(n) {
  let k = n;
  k += 1;
  return `${k}`;
}

let exportedCounter = 0;
export { exportedCounter };
export function bumpExported() {
  exportedCounter++;
}

export function readOnRightSide(values) {
  const step = 2;
  let offset = values.length;
  let x = 1;
  x += step;
  x -= offset;
  return x;
}

export function readInOwnUpdate(n) {
  let acc = n;
  acc *= acc;
}
