function overwrittenInit(user) {
  let text = 'unknown'; // expect: no-useless-assignment
  text = user.name;
  return text;
}

function overwrittenAssign(a, b) {
  let total;
  total = a; // expect: no-useless-assignment
  total = b;
  return total;
}

function overwrittenAfterUnrelated(a) {
  let value = compute(a); // expect: no-useless-assignment
  log('start');
  value = other(a);
  return value;
}

function neverReadBeforeReturn(input) {
  let result = input;
  if (!result) {
    result = 'empty'; // expect: no-useless-assignment
    return null;
  }
  return result;
}

function neverReadAtEnd(items) {
  let count = 0;
  for (const item of items) {
    count += item;
  }
  report(count);
  count = 0; // expect: no-useless-assignment
}

function neverReadAfterThrow(x) {
  let state = x;
  use(state);
  state = 'failed'; // expect: no-useless-assignment
  throw new Error('bad');
}

function parameterOverwritten(options) {
  use(options);
  options = defaults(); // expect: no-useless-assignment
  options = other();
  return options;
}

function nestedBlockEnd(flag) {
  let mode = 'a';
  use(mode);
  if (flag) {
    {
      mode = 'b'; // expect: no-useless-assignment
    }
  }
}

function varDeclarator(a) {
  var v = a + 1; // expect: no-useless-assignment
  v = a + 2;
  return v;
}

const arrow = (a) => {
  let r = a; // expect: no-useless-assignment
  r = a * 2;
  return r;
};

// ---- compliant ----

function readInBetween(a) {
  let x = a;
  use(x);
  x = 2;
  return x;
}

function selfReference(a) {
  let x = a;
  x = x + 1;
  return x;
}

function compoundAssignment(a) {
  let x = a;
  x += 1;
  return x;
}

function inLoop(items) {
  let last = null;
  for (const item of items) {
    use(last);
    last = item;
  }
}

function loopCarried(items) {
  let prev;
  let i = 0;
  while (i < items.length) {
    if (prev) use(prev);
    prev = items[i];
    i++;
  }
}

function breakBetween(items) {
  let found = null;
  for (const item of items) {
    found = item;
    if (item.ok) break;
    found = null;
  }
  return found;
}

function continueBetween(items) {
  let current;
  for (const item of items) {
    current = item;
    if (!item) continue;
    current = null;
  }
  return current;
}

function captured(a) {
  let x = a;
  const read = () => x;
  x = 2;
  return read();
}

function capturedByLaterFunction(a) {
  let x = a;
  x = 2;
  function inner() {
    return x;
  }
  return inner;
}

function insideTry(a) {
  let x = a;
  try {
    x = risky();
    x = other();
  } catch (e) {
    use(x);
  }
  return x;
}

function afterTryFinally(a) {
  let x = a;
  try {
    x = 1;
    step();
  } finally {
    use(x);
  }
}

function usesArguments(a) {
  a = 1;
  return arguments[0];
}

function conditionalOverwrite(a, flag) {
  let x = a;
  if (flag) {
    x = 2;
  }
  return x;
}

function switchCase(kind) {
  let label = 'none'; // expect: no-useless-assignment
  switch (kind) {
    case 1:
      label = 'one';
      break;
    default:
      label = 'other';
  }
  return label;
}

function neverReadIsUnusedVars() {
  let unused = 1;
  unused = 2;
}

function declaredLater(a) {
  let first = a, second = first;
  first = 3;
  return first + second;
}

function writeInThenReadInElse(a, flag) {
  let x = a;
  if (flag) {
    x = 1; // expect: no-useless-assignment
  } else {
    use(x);
  }
}

function labeled(a) {
  let x = a;
  outer: {
    x = 1;
    if (a) break outer;
    x = 2;
  }
  return x;
}

function destructuringOverwrite(a) {
  let x = a;
  [x] = list();
  return x;
}

function classField(a) {
  let x;
  class Box {
    value = x;
  }
  x = a;
  return new Box();
}

let topLevel = 1;
topLevel = 2;
use(topLevel);

function simpleOverwrite(o) {
  let y = o; // expect: no-useless-assignment
  y = 1;
  return y + 1;
}

function bothBranches(a) {
  let t = Infinity; // expect: no-useless-assignment
  if (a === 0) {
    t = 1;
  } else {
    t = compute(a);
  }
  return t;
}

function branchReturns(a) {
  let t = 0; // expect: no-useless-assignment
  if (a) {
    t = 1;
  } else {
    return null;
  }
  return t;
}

function elseIfChainFull(a, b) {
  let t = 0; // expect: no-useless-assignment
  if (a) t = 1;
  else if (b) t = 2;
  else t = 3;
  return t;
}

function oneBranchOnly(a) {
  let t = 0;
  if (a) {
    t = 1;
  }
  return t;
}

function testReads(a) {
  let t = 0;
  if (t > a) {
    t = 1;
  } else {
    t = 2;
  }
  return t;
}

function branchReadsFirst(a) {
  let t = 0;
  if (a) {
    use(t);
    t = 1;
  } else {
    t = 2;
  }
  return t;
}

function elseIfChainOpen(a, b) {
  let t = 0;
  if (a) t = 1;
  else if (b) t = 2;
  return t;
}

function branchWithLoopBreak(a, list) {
  let t = 0;
  for (const x of list) {
    if (a) {
      break;
    } else {
      t = x;
    }
  }
  return t;
}
