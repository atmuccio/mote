function parse(input, undefined) { // expect: no-shadow-restricted-names
  return input === undefined;
}

function compute() {
  var NaN = 0; // expect: no-shadow-restricted-names
  let Infinity = 1e9; // expect: no-shadow-restricted-names
  return NaN + Infinity;
}

try {
  run();
} catch (eval) { // expect: no-shadow-restricted-names
  report(eval);
}

function arguments() {} // expect: no-shadow-restricted-names
class NaN2 {}
const { undefined: missing } = options;
const { NaN: nan } = options; // renamed binding: compliant
const [eval2] = list;
var { Infinity: big } = limits;

function destructured({ eval }) { // expect: no-shadow-restricted-names
  return eval;
}

const shadow = (NaN) => NaN * 2; // expect: no-shadow-restricted-names

// Compliant: `var undefined;` without an initializer that is never assigned.
function guard() {
  var undefined;
  return undefined;
}

// Flagged: assigned later.
function broken() {
  var undefined; // expect: no-shadow-restricted-names
  undefined = 5;
  return undefined;
}

// Compliant: the classic IIFE guard, `undefined` never receives an argument.
(function (window, document, undefined) {
  window.x = undefined;
})(window, document);

// Flagged: the IIFE passes a value for `undefined`.
(function (undefined) { // expect: no-shadow-restricted-names
  return undefined;
})(42);

// Compliant: property names and member accesses are not bindings.
const values = { undefined: 1, NaN: 2, eval: 3, arguments: 4 };
values.NaN = Number.NaN;
function usesArguments() {
  return arguments.length;
}
class Numbers {
  NaN() {}
  get Infinity() { return 1; }
}
