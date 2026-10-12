if (false) {} // expect: no-constant-condition
if (true) {} // expect: no-constant-condition
if (0) {} // expect: no-constant-condition
if ('text') {} // expect: no-constant-condition
if ([]) {} // expect: no-constant-condition
if ({}) {} // expect: no-constant-condition
if (function () {}) {} // expect: no-constant-condition
if (() => 1) {} // expect: no-constant-condition
if (typeof x) {} // expect: no-constant-condition
if (void x) {} // expect: no-constant-condition
if (!true) {} // expect: no-constant-condition
if (undefined) {} // expect: no-constant-condition
if (1 + 2) {} // expect: no-constant-condition
if (`text`) {} // expect: no-constant-condition
if (`${a}text`) {} // expect: no-constant-condition
if (`${a}`) {}
if (/re/) {} // expect: no-constant-condition
if (null || false) {} // expect: no-constant-condition
if (x || {}) {}
if (x) {}
if (!x) {}
if (x === 1) {}
if (typeof x === 'string') {}
if (a, true) {} // expect: no-constant-condition
const v = 0 ? a : b; // expect: no-constant-condition
const w = x ? a : b;
while (true) {}
do {} while (false);
for (;;) {}
for (; true; ) {}
while (1) { break; }
function f(undefined) {
  if (undefined) {}
}
if (DEBUG) {}
if (`a${b}c` === x) {}
