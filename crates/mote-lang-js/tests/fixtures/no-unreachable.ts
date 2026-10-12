export function f(x: number): number {
  return x;
  interface Later { a: number }
  type Alias = string;
}

export function g(x: number): never {
  throw new Error(String(x));
  enum E { A } // expect: no-unreachable
}

export namespace N {
  export function h(): void {
    return;
    let z = 1; // expect: no-unreachable
  }
}
