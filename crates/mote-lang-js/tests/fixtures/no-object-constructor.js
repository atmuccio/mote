const a = new Object(); // expect: no-object-constructor
const b = Object(); // expect: no-object-constructor
const c = new Object; // expect: no-object-constructor
const d = Object(value);
const e = new Object(value);
const f = Object.create(null);
const g = window.Object();
function local() {
  const Object = function () {};
  return new Object();
}
class Thing { make() { return new Object(); } } // expect: no-object-constructor
