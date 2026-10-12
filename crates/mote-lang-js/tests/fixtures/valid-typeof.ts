export function isNum(x: unknown): boolean {
  return typeof x === 'number' || typeof x === 'integer'; // expect: valid-typeof
}
