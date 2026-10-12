const biggest = Math(a, b); // expect: no-obj-calls
const parsed = JSON(text); // expect: no-obj-calls
const copy = new JSON(); // expect: no-obj-calls
const r = Reflect(target); // expect: no-obj-calls
const fmt = new Intl(); // expect: no-obj-calls
Atomics(); // expect: no-obj-calls

// Compliant.
const max = Math.max(a, b);
const obj = JSON.parse(text);
const nf = new Intl.NumberFormat();
function local(Math) {
  return Math(1);
}
export { biggest, parsed, copy, r, fmt, max, obj, nf, local };
