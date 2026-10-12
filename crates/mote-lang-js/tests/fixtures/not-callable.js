'use strict' // expect: not-callable
(function () {
  run();
})();

function literals(a) {
  const message = 'hello'
  ;(function () {})();
  [1, 2](a); // expect: not-callable
  null(); // expect: not-callable
  undefined(a); // expect: not-callable
  new 5(); // expect: not-callable
  ({})(); // expect: not-callable
  `text`(); // expect: not-callable
  /re/(); // expect: not-callable
  return message;
}

class Point {
  constructor(x) {
    this.x = x;
  }
}
const p = Point(1); // expect: not-callable

const make = (x) => ({ x });
const q = new make(1); // expect: not-callable

async function load() {}
const r = new load(); // expect: not-callable

function* gen() {}
const s = new gen(); // expect: not-callable

// ---- compliant ----
const ok1 = new Point(1);
const ok2 = make(2);
function Legacy() {}
const ok3 = new Legacy();
const ok4 = tag`text`;
let Swapped = class {};
Swapped = function () {};
const ok5 = Swapped();
function shadow(undefined) {
  return undefined();
}
const ok6 = (0, make)(1);
export { p, q, r, s, ok1, ok2, ok3, ok4, ok5, ok6, shadow };
