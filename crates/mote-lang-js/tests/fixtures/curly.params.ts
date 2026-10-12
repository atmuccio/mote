// mote-test: params single_line_if=false
function g(x: number | undefined): number {
  if (x === undefined) return 0; // expect: curly
  if (x > 1) {
    return 1;
  }
  return x;
}
