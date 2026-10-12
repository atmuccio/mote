function save(item) {
  store.put(item);
  return; // expect: no-useless-return
}
function early(item) {
  if (!item) {
    return;
  }
  store.put(item);
}
function onlyReturn() {
  return;
}
function withValue() {
  return 1;
}
const arrow = () => {
  run();
  return; // expect: no-useless-return
};
const method = {
  run() {
    work();
    return; // expect: no-useless-return
  },
};
function inLoop(items) {
  for (const i of items) {
    if (i) return;
  }
}
function inSwitch(x) {
  switch (x) {
    case 1:
      return;
  }
}
function* gen() {
  yield 1;
  return; // expect: no-useless-return
}
function tryFinally() {
  try {
    run();
    return;
  } finally {
    cleanup();
  }
}
function nested() {
  function inner() {
    return;
  }
  inner();
}
for (const item of items) {
  store.put(item);
  continue; // expect: no-useless-return
}
for (let i = 0; i < n; i++) {
  if (skip(i)) {
    log(i);
    continue;
  }
  work(i);
}
while (queue.length) {
  const next = queue.shift();
  if (next) {
    handle(next);
  }
  continue; // expect: no-useless-return
}
do {
  step();
  continue; // expect: no-useless-return
} while (more());
for (const key in table) {
  check(key);
  continue; // expect: no-useless-return
}
outer: for (const row of rows) {
  for (const cell of row) {
    visit(cell);
    continue outer;
  }
}
for (const item of items) {
  continue;
}
for (const item of items) continue;
for (const item of items) {
  switch (item.kind) {
    case 1:
      one(item);
      continue;
    case 2:
      two(item);
  }
}
for (const item of items) {
  try {
    run(item);
    continue;
  } catch (e) {
    fail(e);
  }
  retry(item);
}
