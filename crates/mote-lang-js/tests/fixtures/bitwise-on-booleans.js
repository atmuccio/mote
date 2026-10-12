function test(x, y, done, failed, list, obj) {
  if (x > 0 & y > 0) use(x); // expect: bitwise-on-booleans
  if (!done | failed === true) use(y); // expect: bitwise-on-booleans
  const both = (x in obj) & (list instanceof Array); // expect: bitwise-on-booleans
  // Compliant.
  if (x > 0 && y > 0) use(x);
  const bits = x & 0xff;
  const mask = (x > 0) | 0;
  const either = (x > 0) ^ (y > 0);
  let changed = false;
  changed |= x !== y;
  const flags = done | failed;
  return { both, bits, mask, either, changed, flags };
}
export { test };
