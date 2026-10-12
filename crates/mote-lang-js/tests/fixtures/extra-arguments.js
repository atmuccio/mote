import { external } from './external';

function greet(name) {
  return `Hello ${name}`;
}

greet('Ada', 'Lovelace'); // expect: extra-arguments
greet('Ada');
greet();

const add = (a, b) => a + b;
add(1, 2, 3); // expect: extra-arguments
add(1, 2);

const format = function (value) {
  return String(value);
};
format(1, 'extra', 'more'); // expect: extra-arguments

function noParams() {}
noParams(1); // expect: extra-arguments

// Compliant: rest parameters accept anything.
function log(...messages) {
  return messages.join(' ');
}
log('a', 'b', 'c');

function prefixed(prefix, ...rest) {
  return prefix + rest.length;
}
prefixed('x', 1, 2, 3);

// Compliant: the function reads `arguments`.
function variadic() {
  return arguments.length;
}
variadic(1, 2, 3);

function nestedArguments(a) {
  const inner = () => arguments[1];
  return inner() + a;
}
nestedArguments(1, 2);

// Compliant: spread arguments.
const values = [1, 2, 3];
add(...values);
greet(...values, 'x');

// Compliant: reassigned function variables and `let`.
let handler = (x) => x;
handler(1, 2);
handler = (x, y) => x + y;

function replaced(a) {
  return a;
}
replaced(1, 2);
replaced = function (a, b) {
  return a + b;
};

// Compliant: unknown callee.
external(1, 2, 3);
someGlobal(1, 2, 3);

// Compliant: a method call, not a call to the local function.
const api = { greet };
api.greet('a', 'b');

// Default parameters count as parameters.
function withDefault(a, b = 2) {
  return a + b;
}
withDefault(1, 2);
withDefault(1, 2, 3); // expect: extra-arguments

// Local function shadowing an outer one.
function outer() {
  function greet(first, last) {
    return first + last;
  }
  return greet('Ada', 'Lovelace');
}
