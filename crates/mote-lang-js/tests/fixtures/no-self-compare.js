if (a === a) {} // expect: no-self-compare
if (a !== a) {} // expect: no-self-compare
if (a == b) {}
if (a.b < a.b) {} // expect: no-self-compare
if (a.b < a.c) {}
if (user.id === other.id) {}
if (user.id === user.id) {} // expect: no-self-compare
if (this.x >= this.x) {} // expect: no-self-compare
if (arr[0] === arr[0]) {} // expect: no-self-compare
if (arr[0] === arr[1]) {}
if (next() === next()) {}
if (Math.random() === Math.random()) {}
if (a + 1 === a + 1) {} // expect: no-self-compare
if (a + 1 === 1 + a) {}
if (i++ === i++) {}
const same = a // expect: no-self-compare
  === a;
// expect-next-line: no-self-compare
const same2 = b ===
  b;
const t = typeof a === typeof b;
if (a?.b === a?.b) {} // expect: no-self-compare
if (a + b) {}
