// Increments and compound assignments are stores; repeated `var` declarations are one variable;
// plain nested blocks are looked into.

function trailingIncrement(ids) {
  let index = 0;
  while (ids[index]) index++;
  register(ids[index]);
  index++; // expect: no-useless-assignment
}

function suffixNeverRead(kind) {
  let label = kind;
  if (known(label)) {
    label += '-series'; // expect: no-useless-assignment
  }
}

function updateOverwritten(n) {
  let total = n;
  use(total);
  total += 1; // expect: no-useless-assignment
  total = 0;
  return total;
}

list.forEach(function (item) {
  let pos = 0;
  while (item[pos]) pos++;
  item.done = pos;
  pos--; // expect: no-useless-assignment
});

function redeclaredLater(flag) {
  if (flag) {
    var mode = read();
    use(mode);
  }
  var mode = 'x'; // expect: no-useless-assignment
  if (other()) {
    mode = 'a';
  } else {
    mode = 'b';
  }
  return mode;
}

function switchRedeclared(key) {
  switch (key) {
    case 'a':
      var target = find('a');
      use(target);
      break;
    case 'b':
      var target = null; // expect: no-useless-assignment
      if (key) {
        target = find('b');
      } else {
        target = find('c');
      }
      use(target);
      break;
  }
}

function redeclarationOverwrites() {
  var v = first(); // expect: no-useless-assignment
  var v = second();
  return v;
}

function bareBlock(points) {
  let length = 0; // expect: no-useless-assignment
  {
    const [a, b] = points;
    length = Math.hypot(a, b);
  }
  return length;
}

// ---- compliant ----

function counterInLoop(items) {
  let count = 0;
  for (const item of items) {
    if (item) count++;
  }
  return count;
}

function whileCounter(n) {
  let i = 0;
  while (i < n) {
    step(i);
    i++;
  }
}

function valueUsed(list) {
  let index = 0;
  const first = list[index++];
  return first + list[index];
}

function returnedUpdate(n) {
  let c = n;
  use(c);
  return c++;
}

const bump = (n) => n++;

function readAfterUpdate(n) {
  let c = n;
  c++;
  return c;
}

function onlyIncremented() {
  let hits = 0;
  hits++;
}

function logicalAssign(value) {
  let v = value;
  use(v);
  v ||= fallback();
}

function updateInTry(n) {
  let c = n;
  use(c);
  try {
    c++;
  } catch (e) {
    report(e);
  }
}

function paramRedeclared(a) {
  var a = a || 1;
  use(a);
  a++;
}

function redeclarationReads() {
  var s = first();
  var s = s + 1;
  return s;
}

function redeclarationWithoutInit() {
  var t = first();
  var t;
  return t;
}

function blockReadsFirst(v0) {
  let v = v0;
  {
    use(v);
    v = 2;
  }
  return v;
}

function blockConditional(a) {
  let v = 1;
  {
    if (a) v = 2;
  }
  return v;
}

function capturedCounter(items) {
  let n = 0;
  items.forEach(() => n++);
  return n;
}

function capturedRead() {
  let n = 0;
  const get = () => n;
  n++;
  return get;
}

function sequenceUpdate(a) {
  let x = a;
  let y = a;
  use(x, y);
  x++, y++;
}

function forUpdate(n) {
  for (let i = 0; i < n; i++) {
    step(i);
  }
}

function varRedeclaredAsFunction() {
  var helper = 1;
  use(helper);
  function helper() {}
  helper++;
}

function caseBreaksOut(key, row) {
  switch (key) {
    case 'left':
      var width = row.length;
      use(width);
      break;
    case 'right':
      var width = row.length; // expect: no-useless-assignment
      step(row);
      break;
  }
}

function caseBreakThenRead(key, row) {
  let size = 0;
  switch (key) {
    case 'a':
      size = row.length;
      break;
    case 'b':
      size = 1;
  }
  return size;
}

function caseBreakInLoop(keys) {
  let last = null;
  for (const key of keys) {
    switch (key) {
      case 'x':
        use(last);
        last = key;
        break;
    }
  }
}

function caseConditionalBreak(key, row) {
  let n = 0;
  switch (key) {
    case 'a':
      n = row.length;
      if (row.ok) break;
      n = 2;
      break;
  }
  return n;
}

function readOnRightSide(a) {
  let delta = a;
  use(delta);
  delta = a * 2; // expect: no-useless-assignment
  delta = a * 3;
  let total = 0;
  total += delta;
  return total;
}

const lowered = (names) => names.map((name) => (name = name.toLowerCase())); // expect: no-useless-assignment
