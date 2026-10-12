if (x === -0) {} // expect: no-compare-neg-zero
if (-0 == x) {} // expect: no-compare-neg-zero
if (x < -0) {} // expect: no-compare-neg-zero
if (x !== -0.0) {} // expect: no-compare-neg-zero
if (x === 0) {}
if (Object.is(x, -0)) {}
if (x === -1) {}
const y = x + -0;
const z = -0;
if (x === -0n) {}
