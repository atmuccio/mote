Promise = CustomPromise; // expect: no-global-assign
undefined = null; // expect: no-global-assign
function setup() {
  Object = {}; // expect: no-global-assign
  NaN++; // expect: no-global-assign
  [JSON] = [parser]; // expect: no-global-assign
  window = createWindow(); // expect: no-global-assign
}

// ---- compliant ----
if (typeof Map === 'undefined') {
  Map = MapPolyfill;
}
Set = Set || SetPolyfill;
if (!window.WeakMap) WeakMap = WeakMapPolyfill;
function local() {
  let Symbol = 1;
  Symbol = 2;
  return Symbol;
}
var Reflect2 = 1;
location = '/home';
console = fakeConsole;
window.Promise = CustomPromise;
const value = Array.isArray(list) ? Array.from(list) : [];
export { setup, local, value, Reflect2 };
