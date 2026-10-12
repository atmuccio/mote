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
