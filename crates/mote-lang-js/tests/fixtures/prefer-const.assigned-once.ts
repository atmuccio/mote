declare function load(): number;
declare function use(value: unknown): void;

export function simple(): number {
  let result: number;
  const other = 1;
  result = load() + other; // expect: prefer-const
  return result;
}

export function spec(): void {
  let routerStub: { navigate: () => void };
  let second;
  routerStub = { navigate: () => undefined }; // expect: prefer-const
  second = routerStub; // expect: prefer-const
  use(second);
}

export function afterEarlyExit(flag: boolean): number {
  let value;
  if (flag) {
    return 0;
  }
  value = load(); // expect: prefer-const
  return value;
}

export function closureAfter(): () => number {
  let value;
  value = load(); // expect: prefer-const
  return () => value;
}

// Compliant.
export function assignedTwice(): number {
  let value;
  value = load();
  value = value + 1;
  return value;
}

export function readBefore(): number {
  let value: number | undefined;
  use(value);
  value = load();
  return value;
}

export function inBranch(flag: boolean): number {
  const value = 0;
  let other;
  if (flag) {
    other = load();
  }
  return value + (other ?? 0);
}

export function readsInRhs(): number {
  let value: number | undefined;
  value = (value ?? 0) + 1;
  return value;
}

export function closureBefore(): number {
  const value = 0;
  let later: number;
  const get = () => later;
  later = load();
  return get() + value;
}

export function hoisted(): number {
  let later: number;
  const early = read();
  later = load();
  function read(): number {
    return later;
  }
  return early + later;
}

export function inTry(): number {
  let value;
  try {
    value = load();
  } catch {
    return 0;
  }
  return value;
}

export function compound(): number {
  let value: number;
  value = 1;
  value += 1;
  return value;
}

export function multiple(): number {
  let a, b;
  a = load();
  b = load();
  return a + b;
}

export function destructuringWrite(): number {
  let value;
  [value] = [load()];
  return value;
}

export function neverAssigned(): number | undefined {
  let value: number | undefined;
  return value;
}
