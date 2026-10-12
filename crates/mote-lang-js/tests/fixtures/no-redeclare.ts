export function overload(value: string): string;
export function overload(value: number): number;
export function overload(value: string | number): string | number {
  return value;
}

export class Merged {}
export interface Merged {
  extra: number;
}

export function withNamespace(): void {}
export namespace withNamespace {
  export const version = 1;
}

enum Color {
  Red,
}
enum Color {
  Blue = 1,
}

interface Box {
  a: number;
}
interface Box {
  b: number;
}

declare function ambient(): void;
declare function ambient(x: number): void;

declare global {
  var injected: number;
  var injected: number;
}

export function run(): void {
  function helper(): number {
    return 1;
  }
  function helper(): number { // expect: no-redeclare
    return 2;
  }
  helper();
}

export { Color };
