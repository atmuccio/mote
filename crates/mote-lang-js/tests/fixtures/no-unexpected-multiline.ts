function generic<T>(value: T): T {
  return value;
}

export function positives(a: number, list: number[]): unknown {
  const x = generic<number>
  (a); // expect: no-unexpected-multiline

  const y = list!
  [0]; // expect: no-unexpected-multiline

  return [x, y];
}

// ---- compliant ----
export function negatives(a: number, list: number[]): unknown {
  const x = generic<
    number
  >(a);
  const y = (list as number[])[0];
  const z = generic<number>(a) as
    number;
  const w = generic<number>
      (a);
  return [x, y, z, w];
}

function decorate(_target: unknown) {
  return (_: unknown) => undefined;
}

export class Service {
  @decorate
  (1)
  run(): void {}
}
