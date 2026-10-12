Array.prototype.last = function () { // expect: no-extend-native
  return this[this.length - 1];
};
String.prototype['capitalize'] = function () {}; // expect: no-extend-native
Object.defineProperty(Array.prototype, 'first', { value() { return this[0]; } }); // expect: no-extend-native
Object.defineProperties(Date.prototype, { a: { value: 1 } }); // expect: no-extend-native
Object.assign(Promise.prototype, { done() {} }); // expect: no-extend-native
Number.prototype.times = (n) => n; // expect: no-extend-native

if (typeof window !== 'undefined') {
  Element; // not a built-in of the language
  Array.prototype.shuffle = shuffle; // expect: no-extend-native
}

// Compliant: guarded polyfills.
if (!Array.prototype.at) {
  Array.prototype.at = function (i) { return this[i]; };
}
if (!('includes' in String.prototype)) {
  Object.defineProperty(String.prototype, 'includes', { value: includes });
}
Array.prototype.flat = Array.prototype.flat || flat;
Array.prototype.flatMap ??= flatMap;
typeof Array.prototype.find === 'function' || (Array.prototype.find = find);
Array.prototype.findLast ? null : (Array.prototype.findLast = findLast);
function install() {
  if (Object.prototype.hasOwn) return;
  Object.prototype.hasOwn = hasOwn;
}

// Compliant: not built-ins, or not their prototypes.
function Local() {}
Local.prototype.run = function () {};
MyArray.prototype.last = last;
Array.from = from;
const x = Array.prototype.slice.call(arguments);
