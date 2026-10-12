export const pattern: RegExp = /^v{1}[0-9]+$/; // expect: redundant-regex-syntax
export const fine: RegExp = /^v[0-9]+$/;

export function check(input: string): boolean {
  const sep: RegExp = /[a-z]+(-|_)[a-z]+/; // expect: redundant-regex-syntax
  return (/(?<id>\d+)/ as RegExp).test(input) && sep.test(input); // expect: redundant-regex-syntax
}
