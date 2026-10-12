function greet(name) {
  name = name || 'guest'; // expect: no-param-reassign
  return name;
}
function counter(n) {
  n++; // expect: no-param-reassign
  n += 2; // expect: no-param-reassign
  return n;
}
function destructured({ a, b }, [c]) {
  a = 1; // expect: no-param-reassign
  [c] = [2]; // expect: no-param-reassign
  ({ b } = {}); // expect: no-param-reassign
  return a + b + c;
}
function loopTarget(item, items) {
  for (item of items) { // expect: no-param-reassign
    use(item);
  }
}
function rest(...args) {
  args = []; // expect: no-param-reassign
  return args;
}
function withDefault(x = 1) {
  return x;
}
const arrow = (value) => {
  value = value.trim(); // expect: no-param-reassign
  return value;
};
const closure = (value) => () => {
  value = 0; // expect: no-param-reassign
};
function members(options) {
  options.enabled = true;
  options.list.push(1);
  options['key'] = 2;
  delete options.old;
  return options;
}
function shadow(x) {
  {
    let x = 2;
    x = 3;
    use(x);
  }
  return x;
}
function logical(opts) {
  opts ??= {}; // expect: no-param-reassign
  return opts;
}
class K {
  method(p) {
    p = 1; // expect: no-param-reassign
    return p;
  }
  set value(v) {
    this._v = v;
  }
}
function read(a) {
  const b = a;
  return b;
}
