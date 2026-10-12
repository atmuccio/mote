function f(user, isValid, flags, size, a) {
  const name = user.nickname || user.nickname; // expect: identical-operands
  if (isValid && isValid) use(name); // expect: identical-operands
  const masked = flags & flags;
  const cleared = flags ^ flags; // expect: identical-operands
  const rem = size % size; // expect: identical-operands
  const one = size / size; // expect: identical-operands
  const v = a.b ?? a.b; // expect: identical-operands
  // Compliant.
  const other = user.nickname || user.fullName;
  const finite = size - size === 0;
  const doubled = size + size;
  const squared = size * size;
  const calls = next() || next();
  const lits = 1 | 1;
  const strings = 'a' || 'a';
  return { cleared, rem, masked, one, v, other, finite, doubled, squared, calls, lits, strings };
}
export { f };
