import { run } from './run.js';

function schedule() {
  let timeout; // expect: no-unassigned-vars
  setTimeout(run, timeout);
}

function many() {
  var a, b = 2, c; // expect: no-unassigned-vars, no-unassigned-vars
  return a + b + c;
}

let moduleLevel; // expect: no-unassigned-vars
console.log(moduleLevel);

function inClosure() {
  let value; // expect: no-unassigned-vars
  return () => value;
}

// ---- compliant ----

function assignedLater() {
  let x;
  x = 1;
  return x;
}

function assignedInCallback(items) {
  let found;
  items.forEach((item) => {
    found = item;
  });
  return found;
}

function destructured(obj) {
  let a;
  ({ a } = obj);
  let b;
  [b] = obj.list;
  return a + b;
}

function loopHeads(obj, list) {
  let key;
  for (key in obj) {
  }
  var item;
  for (item of list) {
  }
  for (let k in obj) {
    run(k);
  }
  return key + item;
}

function incremented() {
  let n;
  n++;
  return n;
}

function neverRead() {
  let unused;
}

function hoisted() {
  use();
  var v;
  function use() {
    v = 1;
  }
  return v;
}

export let exported;
console.log(exported);

function shadowParam(p) {
  var p;
  return p;
}
