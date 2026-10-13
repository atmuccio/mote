if (!!user) {} // expect: no-extra-boolean-cast
while (Boolean(next)) {} // expect: no-extra-boolean-cast
do {} while (!!more); // expect: no-extra-boolean-cast
for (; !!more; ) {} // expect: no-extra-boolean-cast
const v = !!a ? 1 : 2; // expect: no-extra-boolean-cast
const n = !!!a; // expect: no-extra-boolean-cast
const nb = !Boolean(a); // expect: no-extra-boolean-cast
const bb = Boolean(!!a); // expect: no-extra-boolean-cast
const nw = new Boolean(!!a); // expect: no-extra-boolean-cast
const hasUser = !!user;
const asBool = Boolean(user);
function f() {
  return !!user;
}
const obj = { flag: !!user };
const t = cond ? !!a : !!b;
for (let i = !!start; i; ) {}
if (!!a && b) {}
if (x.filter(Boolean)) {}
function shadow(Boolean) {
  if (Boolean(x)) {}
}
if (Boolean()) {}
if (Boolean(...args)) {}
const p1 = !!!(a || b); // expect: no-extra-boolean-cast
const p2 = !Boolean(a || b); // expect: no-extra-boolean-cast
const p3 = !Boolean((a) || b); // expect: no-extra-boolean-cast
const p4 = !!(a = b) ? 1 : 2; // expect: no-extra-boolean-cast
const p5 = Boolean(x => x) ? 1 : 2; // expect: no-extra-boolean-cast
if (Boolean((a, b))) {} // expect: no-extra-boolean-cast
const p6 = !Boolean((c)); // expect: no-extra-boolean-cast
