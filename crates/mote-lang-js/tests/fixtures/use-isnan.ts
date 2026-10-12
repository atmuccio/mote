export function check(n: number): boolean {
  return n === (NaN as number); // expect: use-isnan
}
