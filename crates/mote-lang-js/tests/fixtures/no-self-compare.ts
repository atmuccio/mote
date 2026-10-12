function check(x: number, y: number): boolean {
  if ((x as number) === x) {}
  if (x! === x!) {} // expect: no-self-compare
  return x === y;
}
