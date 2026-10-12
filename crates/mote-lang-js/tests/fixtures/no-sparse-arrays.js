const colors = ['red', , 'blue']; // expect: no-sparse-arrays
const lead = [, 'a']; // expect: no-sparse-arrays
const two = [1, , , 2]; // expect: no-sparse-arrays
const trailing = [1, 2,];
const ok = ['red', undefined, 'blue'];
const [, second] = pair;
[, b] = pair;
for (const [, v] of entries) {}
function f([, x]) { return x; }
const nested = [[1, , 2]]; // expect: no-sparse-arrays
const multi = [
  1,
  , // expect: no-sparse-arrays
  3,
];
