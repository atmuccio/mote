export function sum(xs: number[]): number {
  let total = 0;
  for (let i: number = xs.length - 1; i >= 0; i++) { // expect: for-direction
    total += xs[i];
  }
  return total;
}
