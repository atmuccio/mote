import { used, unusedImport } from './mod'; // expect: no-unused-vars
import DefaultUnused from './mod2'; // expect: no-unused-vars
import * as ns from './ns';
import './side-effect';
ns.run();
used();

const unusedConst = 1; // expect: no-unused-vars
let assignedOnly; // expect: no-unused-vars
assignedOnly = 5;
const _ignored = 2;
export const exported = 3;
const usedLater = 4;
export default usedLater;
const reExported = 5;
export { reExported as renamed };

function unusedFn() {} // expect: no-unused-vars
function recursive(n) { return n ? recursive(n - 1) : 0; } // expect: no-unused-vars
function hoisted() { return 1; }
hoistedCaller();
function hoistedCaller() { return hoisted(); }

class UnusedClass {} // expect: no-unused-vars
class UsedClass {}
new UsedClass();
const Expr = class Named {};
new Expr();
const fnExpr = function named() {};
fnExpr();

export function params(a, b, c) { // expect: no-unused-vars
  return b;
}

export function callbacks(items) {
  items.forEach((item, index) => use(index));
  items.map((item) => 1); // callbacks must accept what the caller passes
  return items.filter(function (x) { return true; });
}

export function destructuring(obj, arr) {
  const { a, b, ...rest } = obj;
  const { c, d } = obj; // expect: no-unused-vars
  const [first, second] = arr;
  const [x, y] = arr; // expect: no-unused-vars
  const { e: { f } } = obj; // expect: no-unused-vars
  return [rest, c, second, x];
}

export function closure() {
  let count = 0;
  return () => { count += 1; return count; };
}

export function counter() {
  let n = 0; // expect: no-unused-vars
  n++;
}

export function loops(list) {
  for (const item of list) { // expect: no-unused-vars
    use();
  }
  for (const key in list) { use(key); }
}

export async function* gen() {
  const value = await fetchIt(); // expect: no-unused-vars
  yield 1;
}

export function optional(obj) {
  const maybe = obj?.a?.b;
  return maybe ?? null;
}

try { risky(); } catch (err) { /* catch parameters are ignored */ }

export function implicitArgs() {
  return function (a, b) { return arguments.length; };
}

export function writeOnly() {
  let x; // expect: no-unused-vars
  x = 1;
  x = 2;
}

const selfRef = () => selfRef(); // expect: no-unused-vars

export const obj = {
  set value(v) {},
  method(unusedParam) {}, // object members often implement a contract
  withDefault(a = 1) { return 2; },
};

export class Service {
  constructor(dep) {} // expect: no-unused-vars
  handle(_event, data) { return data; }
  withRest(first, ...rest) { return first; }
}

export function shadow() {
  const value = 1; // expect: no-unused-vars
  return function inner(value) { return value; };
}

export function typeofUse() {
  const probe = 1;
  return typeof probe;
}

export function labeled() {
  outer: for (;;) { break outer; }
}

export class Plain {
  run(input) {} // an empty hook meant to be overridden
  stop(reason) { return 0; } // expect: no-unused-vars
}

export class Derived extends Plain {
  run(input) {}
  constructor(a, b) { super(); this.a = a; } // expect: no-unused-vars
}

const assigned = (a, b) => a; // expect: no-unused-vars
export { assigned };

export function returnsCallback() {
  return (event) => 1;
}
