function sum() {
  return Array.prototype.reduce.call(arguments, (a, b) => a + b, 0); // expect: prefer-rest-params
}

function first() {
  return arguments[0]; // expect: prefer-rest-params
}

function forward(fn) {
  return fn.apply(this, arguments); // expect: prefer-rest-params
}

function viaArrow() {
  const all = () => Array.from(arguments); // expect: prefer-rest-params
  return all();
}

const obj = {
  method() {
    return [].slice.call(arguments, 1); // expect: prefer-rest-params
  },
};

class Logger {
  log() {
    console.log.apply(console, arguments); // expect: prefer-rest-params
  }
}

// Compliant.
function count() {
  return arguments.length; // only the count
}

function rest(...args) {
  return args[0];
}

function shadowed(arguments_) {
  const arguments2 = arguments_;
  return arguments2;
}

function localName() {
  const args = { length: 0 };
  return args;
}

class Fields {
  static {
    this.x = 1;
  }
}

export { sum, first, forward, viaArrow, obj, Logger, count, rest, shadowed, localName, Fields };
