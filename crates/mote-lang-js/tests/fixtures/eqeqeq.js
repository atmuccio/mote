if (a == b) {} // expect: eqeqeq
if (a != b) {} // expect: eqeqeq
if (a === b) {}
if (a !== b) {}
if (a == null) {}
if (null != a) {}
if (typeof a == 'string') {}
if ('undefined' != typeof a) {}
if ('a' == 'b') {}
if (1 != 2) {}
if (1 == '1') {} // expect: eqeqeq
const s = a /* == */ == b; // expect: eqeqeq
if (a == undefined) {} // expect: eqeqeq
const t = (a == // expect: eqeqeq
  b);
const u = x?.y == z; // expect: eqeqeq
