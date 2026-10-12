function onError() {} // expect: no-empty-function
const handlers = {
  close() {}, // expect: no-empty-function
  open: function () {}, // expect: no-empty-function
  run: () => {}, // expect: no-empty-function
};
function documented() {
  // Errors are reported by the caller.
}
const noop = () => {};
function noop2() {}
const NOOP = function () {};
function doNothing() {}
items.forEach(() => {}); // expect: no-empty-function
promise.catch(function () {}); // expect: no-empty-function
function withDefault(callback = () => {}) {
  return callback;
}
const expressionBody = () => undefined;
class Service {
  constructor() {}
  dispose() {} // expect: no-empty-function
  get value() { return 1; }
  static init() {} // expect: no-empty-function
  #secret() {} // expect: no-empty-function
  field = () => {}; // expect: no-empty-function
}
class Derived extends Service {
  constructor() {
    super();
  }
}
function* gen() {} // expect: no-empty-function
async function load() {} // expect: no-empty-function
function withDirective() {
  'use strict';
}
obj.onload = function () {}; // expect: no-empty-function
obj.noop = function () {};
