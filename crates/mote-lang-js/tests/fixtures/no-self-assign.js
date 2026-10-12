let a = 1, b = 2, c = 3;
a = a; // expect: no-self-assign
a = b;
a += a;
a ||= a; // expect: no-self-assign
a ??= b;
const obj = { x: 1, y: { z: 2 } };
obj.x = obj.x;
obj.x = obj.y;
obj[a] = obj[a];
obj[a++] = obj[a++];
getObj().x = getObj().x;
[a, b] = [a, b]; // expect: no-self-assign, no-self-assign
[a, b] = [b, a];
[a, , c] = [a, b, c]; // expect: no-self-assign, no-self-assign
[a, b] = [...rest, b];
[a = 1] = [a];
({ a, b } = { a, b }); // expect: no-self-assign, no-self-assign
({ a } = { a: b });
({ a: b } = { a: b }); // expect: no-self-assign
({ a } = { ...other, a });
({ a } = { a, a: c });
[obj.x, obj.y] = [obj.x, obj.y]; // expect: no-self-assign, no-self-assign
[obj.x, obj.y] = [obj.y, obj.x];
location.href = location.href;
window.location.href = window.location.href;
canvas.width = canvas.width;
el.scrollTop = el.scrollTop;
el['src'] = el['src'];
picker.items = picker.items;
function setX(x) {
  obj.x = obj.x; // expect: no-self-assign
}
class Counter {
  #n = 0;
  constructor(name) {
    this.name = this.name; // expect: no-self-assign
    this.name = name;
    this.#n = this.#n; // expect: no-self-assign
  }
  reset() {
    this.count = this.count; // expect: no-self-assign
    this.size = this.size;
    this.#m = this.#m;
    const later = () => {
      this.count = this.count; // expect: no-self-assign
    };
    later();
  }
  set size(v) {}
  set #m(v) {}
}
class Derived extends Base {
  refresh() {
    this.items = this.items;
  }
}
const literal = {
  refresh() {
    this.items = this.items;
  },
};
function plain() {
  this.items = this.items;
}
function f(x) {
  x = x; // expect: no-self-assign
  const y = x;
  return y;
}
