function reset() {
  return count = 0; // expect: no-return-assign
}
function explicit() {
  return (this.cache = compute());
}
function nested() {
  return a + (b = 1);
}
items.forEach(i => total += i); // expect: no-return-assign
items.forEach(i => (total += i));
const setter = (v) => this.value = v; // expect: no-return-assign
function inCall() {
  return f(a = 1);
}
function inArrowBlock() {
  items.forEach(i => {
    total += i;
  });
  return total;
}
function compare() {
  return a === b;
}
function inner() {
  return () => {
    x = 1;
  };
}
function inObject() {
  return { a: b = 1 }; // expect: no-return-assign
}
function withComment() {
  return /* note */ (x = 1);
}
function logical() {
  return this._value ??= create(); // expect: no-return-assign
}
function inClass() {
  return class {
    m() { y = 2; }
  };
}
