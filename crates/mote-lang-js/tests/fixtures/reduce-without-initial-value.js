function positives(items, data, isPaid) {
  const total = items.filter(isPaid).reduce((sum, x) => sum + x); // expect: reduce-without-initial-value
  const last = items.filter(Boolean).reduceRight((a, b) => b); // expect: reduce-without-initial-value
  const merged = Object.values(data).reduce((a, b) => ({ ...a, ...b })); // expect: reduce-without-initial-value
  const none = [].reduce((a, b) => a + b); // expect: reduce-without-initial-value
  const maybe = items?.filter(isPaid).reduce((a, b) => a + b); // expect: reduce-without-initial-value
  return [total, last, merged, none, maybe];
}

// ---- compliant ----
function negatives(items, data, isPaid, sum, init) {
  const total = items.filter(isPaid).reduce((acc, x) => acc + x, 0);
  const merged = Object.values(data).reduce((a, b) => ({ ...a, ...b }), {});
  const plain = items.reduce((a, b) => a + b);
  const mapped = items.map(Number).reduce(sum);
  const literal = [1, 2, 3].reduce((a, b) => a + b);
  const spread = items.filter(isPaid).reduce(...init);
  const custom = items.filter().reduce(sum);
  return [total, merged, plain, mapped, literal, spread, custom];
}

positives([], {}, Boolean);
negatives([], {}, Boolean, (a, b) => a + b, []);
