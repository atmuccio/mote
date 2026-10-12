export function f(x: number | undefined): void {
  if (x === undefined) {
    return;
  } else {
    if (x > 1) { // expect: no-lonely-if
      run(x as number);
    }
  }
}
