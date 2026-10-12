export function typed(): number | undefined {
  let value: number | undefined; // expect: no-unassigned-vars
  return value;
}

export function definite(): string {
  let text!: string; // expect: no-unassigned-vars
  return text.trim();
}

export function typeOnly(): void {
  let shape: { a: number };
  type Shape = typeof shape;
  const s: Shape = { a: 1 };
  console.log(s);
}

declare let fromElsewhere: number;
console.log(fromElsewhere);

declare global {
  var injected: string;
}
console.log(injected);

declare module 'lib' {
  let internal: number;
  export function read(): typeof internal;
}

export function viaPromise(): Promise<void> {
  let resolve!: () => void;
  const p = new Promise<void>((r) => {
    resolve = r;
  });
  resolve();
  return p;
}

export function deliberatelyUndefined(): boolean {
  let nothing: undefined | null;
  let empty: void;
  return nothing === undefined && empty === undefined;
}
