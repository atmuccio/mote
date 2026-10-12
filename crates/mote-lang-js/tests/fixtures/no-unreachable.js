function save(record) {
  return store.put(record);
  audit(record); // expect: no-unreachable
}

function load() {
  return read();
  function read() {}
}

function hoisted() {
  return x;
  var x;
}

function withInit() {
  return 1;
  var y = 2; // expect: no-unreachable
}

function afterThrow() {
  throw new Error('x');
  cleanup(); // expect: no-unreachable
}

function afterIfElse(a) {
  if (a) {
    return 1;
  } else {
    throw new Error('no');
  }
  done(); // expect: no-unreachable
}

function onlyIf(a) {
  if (a) {
    return 1;
  }
  done();
}

function loops(items) {
  for (const item of items) {
    if (item) continue;
    use(item);
  }
  while (true) {
    break;
    never(); // expect: no-unreachable
  }
  for (;;) {
    continue;
    never(); // expect: no-unreachable
  }
  return;
}

function inSwitch(x) {
  switch (x) {
    case 1:
      return 'one';
      break;
    case 2:
      return 'two';
      console.log('two'); // expect: no-unreachable
    default:
      break;
  }
  return null;
}

function tryCatch() {
  try {
    return work();
  } catch (e) {
    throw e;
  }
  after(); // expect: no-unreachable
}

function tryNoCatchExit() {
  try {
    return work();
  } catch (e) {
    log(e);
  }
  after();
}

function tryFinally() {
  try {
    work();
  } finally {
    cleanup();
  }
  after();
}

function labeled() {
  outer: {
    break outer;
  }
  after();
  block: {
    if (a) break block;
    stillReachable();
  }
}

function classAfter() {
  return 1;
  class Late {} // expect: no-unreachable
}

function multiLine() {
  return;
  call( // expect: no-unreachable
    a,
    b,
  );
}

const arrow = () => {
  return 1;
  // a comment only
};

function nested() {
  {
    return;
  }
  after(); // expect: no-unreachable
}

throw new Error('top level');
topLevel(); // expect: no-unreachable

function allCasesReturn(x) {
  switch (x) {
    case 1:
      return 'one';
    default:
      return 'other';
  }
  conservative();
}

function tryFinallyExit() {
  try {
    return work();
  } finally {
    cleanup();
  }
  after(); // expect: no-unreachable
}

function infinite() {
  while (true) {
    if (done()) break;
  }
  reachable();
}

function doubleReturn() {
  return 1;
  return 2; // expect: no-unreachable
}
