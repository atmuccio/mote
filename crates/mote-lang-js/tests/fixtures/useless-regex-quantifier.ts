export const ID = /^id-(?:\d){0}$/u; // expect: useless-regex-quantifier
export const OK = /^id-\d{0,4}$/u;
export function test(value: string): boolean {
  return /(?:$)?end/.test(value); // expect: useless-regex-quantifier
}
