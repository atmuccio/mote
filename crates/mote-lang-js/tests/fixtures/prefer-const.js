import { load } from './load';

let total = load() * 2; // expect: prefer-const
export let exported = 1; // expect: prefer-const
let counter = 0;
counter++;
let sum = 0;
sum += 1;
let later;
later = load(); // expect: prefer-const
let reassigned = 1;
reassigned = 2;

let a = 1, b = 2; // expect: prefer-const
let c = 1, d = 2;
d = 3;
let e = 1, f; // not all initialized
f = 2;

let { x, y } = load(); // expect: prefer-const
let { p, q } = load();
q = 1;
let [first, ...rest] = load(); // expect: prefer-const
let obj = {}; // expect: prefer-const
obj.value = 1;
obj.items.push(2);

for (let item of load()) { // expect: prefer-const
  use(item);
}
for (let key in obj) { // expect: prefer-const
  use(key);
}
for (let i = 0; i < 10; i++) {
  use(i);
}
for (let i = 0, n = total; i < n; i++) {
  use(i);
}
for (let item of load()) {
  item = item.trim();
  use(item);
}

function inner() {
  let local = 1; // expect: prefer-const
  let changed = 1;
  const reset = () => {
    changed = 0;
  };
  let swapped = 1;
  [swapped] = [2];
  let destructured = 1;
  ({ destructured } = obj);
  let forTarget;
  for (forTarget of load()) {
    use(forTarget);
  }
  let logical = null;
  logical ??= 1;
  return [local, changed, reset, swapped, destructured, logical];
}

var old = 1;
const fine = 2;
switch (total) {
  case 1: {
    let inCase = 1; // expect: prefer-const
    use(inCase);
    break;
  }
}
