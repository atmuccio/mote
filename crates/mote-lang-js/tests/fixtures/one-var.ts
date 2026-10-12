let width = 0, height = 0; // expect: one-var
let a, b; // expect: one-var
const c = 1;
export const d = 1, e = 2; // expect: one-var
for (let i = 0, n = 10; i < n; i++) {}
for (var j = 0, k = 1; j < k; j++) {}
declare let g1: number, g2: number;
declare global {
  var h1: number, h2: number;
}
function f() {
  var x = 1, y = 2; // expect: one-var
  const [p, q] = [1, 2];
  using r = res(), s = res(); // expect: one-var
  return x + y + p + q;
}
