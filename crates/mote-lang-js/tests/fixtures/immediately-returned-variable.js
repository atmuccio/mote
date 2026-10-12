function label(prefix, name) {
  // expect-next-line: immediately-returned-variable
  const text = `${prefix}-${name}`;
  return text;
}

function fail(reason) {
  // expect-next-line: immediately-returned-variable
  const error = new Error(reason);
  throw error;
}

async function load(url) {
  // expect-next-line: immediately-returned-variable
  let response = await fetch(url);
  return response;
}

function inSwitch(kind) {
  switch (kind) {
    case 'a': {
      // expect-next-line: immediately-returned-variable
      const v = compute(kind);
      return v;
    }
    case 'b':
      // expect-next-line: immediately-returned-variable
      var w = compute(kind);
      return w;
  }
  return null;
}

// Compliant.
function used(x) {
  const y = transform(x);
  log(y);
  return y;
}

function named() {
  const handler = () => 1;
  return handler;
}

function klass() {
  const Impl = class {};
  return Impl;
}

function fn() {
  const f = function () {};
  return f;
}

function destructured(obj) {
  const { a } = obj;
  return a;
}

function two() {
  const a = 1, b = 2;
  return a;
}

function commented(x) {
  const result = transform(x);
  // result is cached by transform
  return result;
}

function selfReferencing() {
  const node = { parent: null, children: [] };
  node.parent = node;
  return node;
}

function recursive() {
  const obj = { get self() { return obj; } };
  return obj;
}

function other(x) {
  const y = x + 1;
  return x;
}

function noValue() {
  let z;
  return z;
}
