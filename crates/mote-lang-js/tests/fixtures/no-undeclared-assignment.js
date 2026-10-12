/* global sharedCounter, legacyConfig: writable */

function total(items) {
  sum = 0; // expect: no-undeclared-assignment
  for (const item of items) sum += item.price;
  return sum;
}

function loop(list) {
  for (i = 0; i < list.length; i++) { // expect: no-undeclared-assignment
    list[i]++;
  }
}

function destructure(pair) {
  [first, second] = pair; // expect: no-undeclared-assignment, no-undeclared-assignment
  return first + second;
}

function keys(obj) {
  for (key in obj) { // expect: no-undeclared-assignment
    console.log(key);
  }
}

function closure(items) {
  found = null; // expect: no-undeclared-assignment
  items.forEach((item) => {
    if (item.ok) found = item;
  });
  return found;
}

// Compliant: declared variables.
function declared(items) {
  let count = 0;
  for (const item of items) count += item;
  return count;
}

// Compliant: known globals and event handlers.
function browser() {
  window = globalThis;
  location = '/home';
  onload = () => {};
  onmessage = (event) => event;
  module = {};
  exports = {};
  name = 'frame';
}

// Compliant: declared in a `/* global */` comment.
function configure() {
  sharedCounter = 1;
  legacyConfig = { debug: true };
}

// Compliant: used across functions (probably a real global).
function setGlobal() {
  appState = {};
}
function readGlobal() {
  return appState;
}

// Compliant: read before written (the global exists elsewhere).
function bump() {
  hits = hits + 1;
  hits = 0;
}

// Compliant: typeof check means the author knows it may be a global.
function guarded() {
  if (typeof polyfill === 'undefined') {
    polyfill = {};
  }
}

// Compliant: top-level assignment (a script setting a global on purpose).
topLevel = 1;
use(topLevel);

// Compliant: member writes and declared outer variables.
let outerVar;
function writesOuter() {
  outerVar = 1;
  window.custom = 2;
  this.value = 3;
}
