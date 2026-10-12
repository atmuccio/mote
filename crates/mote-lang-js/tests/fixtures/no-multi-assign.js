a = b = 0; // expect: no-multi-assign
const x = y = []; // expect: no-multi-assign
a = b = c = d; // expect: no-multi-assign
let z = (w = 1); // expect: no-multi-assign
a = 1;
b = 2;
a += b = 1; // expect: no-multi-assign
f(a = 1);
if ((a = next())) {}
const obj = { k: (a = 1) };
class K {
  field = other = 1; // expect: no-multi-assign
  method() {
    this.x = this.y = null; // expect: no-multi-assign
  }
}
for (i = j = 0; i < 3; i++) {} // expect: no-multi-assign
const arrow = () => (a = 1);
