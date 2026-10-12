function examples(items, index, base, buf, pos, attempts, max) {
  process(items[index], index++); // expect: assignment-in-operand
  const total = base + (delta = compute()); // expect: assignment-in-operand
  call(a = b); // expect: assignment-in-operand
  const list = [count++, 2]; // expect: assignment-in-operand
  const obj = { a: (x = 1) }; // expect: assignment-in-operand
  const s = `${n++}`; // expect: assignment-in-operand
  cond ? (a = 1) : (b = 2); // expect: assignment-in-operand, assignment-in-operand
  ready && (done = true); // expect: assignment-in-operand
  if (++attempts > max) {} // expect: assignment-in-operand
  const v = cache || (cache = build()); // expect: assignment-in-operand

  // Compliant.
  index++;
  a = b = 0;
  a = 1, b = 2;
  const n2 = count++;
  x = y++;
  x += i++;
  buf[pos++] = 0;
  const c = buf[pos--];
  for (let i = 0, j = n++; i < (m = limit()); i++, j = k + (z = 1)) {}
  while ((m = re.exec(s)) !== null) {}
  while (n-- > 0) {}
  do {} while ((line = next()));
  if ((match = pattern.exec(text))) {}
  const cb = () => count++;
  const setter = (v) => void (field = v);
  const ref = (el) => (this.el = el);
  items.forEach((i) => (sum += i));
  return value = compute();
}

class Fields {
  counter = next++;
  constructor(a = b++) {}
}
