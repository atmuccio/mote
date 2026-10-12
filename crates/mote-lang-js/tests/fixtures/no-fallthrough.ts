export function kind(k: number): string {
  let out = '';
  switch (k) {
    case 0:
      out = 'zero';
    case 1: // expect: no-fallthrough
      out += 'one';
      break;
    default:
      out = 'other';
  }
  return out;
}
