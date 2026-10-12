export function largest(sizes: number[]): number[] {
  return sizes.sort(); // expect: sort-without-comparator
}

export function ranks(values: readonly number[], other: Array<number>, third: ReadonlyArray<number>) {
  const copy = values.toSorted(); // expect: sort-without-comparator
  other.sort(); // expect: sort-without-comparator
  third.toSorted(); // expect: sort-without-comparator
  return copy;
}

export function local(): number[] {
  const list: number[] = compute();
  list.sort(); // expect: sort-without-comparator
  return list;
}

// Compliant.
export function ok(names: string[], mixed: (number | string)[], values: number[], m: Map<string, number[]>) {
  names.sort();
  mixed.sort();
  values.sort((a, b) => a - b);
  values.slice().sort((a, b) => a - b);
  values.slice().sort(); // receiver is a call result that is not recognised
  m.get('x')?.sort();
  const typed: Int32Array = new Int32Array(values);
  typed.sort();
  const any: any = values;
  any.sort();
}

declare function compute(): number[];
