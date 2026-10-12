const empty = new Array(); // expect: no-array-constructor
const pair = Array(x, y); // expect: no-array-constructor
const three = new Array(1, 2, 3); // expect: no-array-constructor
const emptyCall = Array(); // expect: no-array-constructor
const sized = new Array(size);
const sizedCall = Array(10);
const spread = new Array(...args);
const literal = [x, y];
const from = Array.from(x);
const opt = Array?.(1, 2);
function local(Array) {
  return new Array(1, 2);
}
const two = new Array(...a, b); // expect: no-array-constructor
