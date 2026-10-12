function handler() {}
handler = () => {}; // expect: no-func-assign
handler();

function counter() {}
counter += 1; // expect: no-func-assign

function destructured() {}
[destructured] = [1]; // expect: no-func-assign
({ x: destructured } = obj); // expect: no-func-assign

let variable = function () {};
variable = () => {};

const named = function inner() {
  inner = null; // expect: no-func-assign
};

function shadow() {
  let shadow = 1;
  shadow = 2;
  return shadow;
}

function param(fn) {
  fn = null;
}

var redeclared;
function redeclared() {}
redeclared = 1;

function used() {}
used.property = 1;
used();
