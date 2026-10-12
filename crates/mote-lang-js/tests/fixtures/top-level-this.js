import { render } from './render.js';

this.app = render(); // expect: top-level-this
const onClick = () => this.toggle(); // expect: top-level-this
export default this; // expect: top-level-this
const nested = () => () => this; // expect: top-level-this
if (ready) {
  console.log(this); // expect: top-level-this
}

function regular() {
  return this.value;
}
const expression = function () {
  return this;
};
const object = {
  method() {
    return this.x;
  },
  get prop() {
    return this.y;
  },
};

class Widget extends Base {
  field = this.compute();
  arrowField = () => this.field;
  static shared = this.name;
  static {
    this.registry = new Map();
  }
  method() {
    const inner = () => this.field;
    return inner();
  }
}

// Compiler-emitted guards test `this` first.
var __helper = (this && this.__helper) || function () {};
const kind = typeof this;
const self = this ? this : globalThis;
const isUndefined = this === undefined;

// A wrapped UMD bundle inspects the environment it is handed.
!function (root, factory) {
  root.lib = factory();
}(this, function () {
  return {};
});
(function (global) {
  global.x = 1;
})(this);
(() => this.run())(); // expect: top-level-this
!function (e, r) {
  e.lib = r();
}('undefined' != typeof self ? self : this, function () {
  return {};
});
