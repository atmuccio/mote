function firstOf<T>(list: string[]): string { // expect: no-unused-vars
  return list[0];
}

const wrap = <K, V>(key: K): K[] => [key]; // expect: no-unused-vars

class Registry {
  private lookup<T>(name: string): string { // expect: no-unused-vars
    return name;
  }

  #find<R>(id: number): number { // expect: no-unused-vars
    return id;
  }

  // Non-private methods are left alone: subclasses may share the signature.
  plain<Q>(n: number): number {
    return n;
  }

  private readonly handler = <E,>(value: number): number => value; // expect: no-unused-vars
}

export class Api {
  private helper<T>(x: number): number { // expect: no-unused-vars
    return x;
  }

  // Public methods of exported classes are part of their API.
  get<T>(key: string): string {
    return key;
  }
}

// ---- compliant ----

export function exported<T>(value: string): string {
  return value;
}

function used<T>(value: T): T {
  return value;
}

function usedInReturn<T>(raw: unknown): T[] {
  return raw as T[];
}

function constrained<T, K extends keyof T>(key: K): K {
  return key;
}

function overloaded<T>(x: string): string;
function overloaded<T>(x: number): number;
function overloaded<T>(x: any): any {
  return x;
}

function underscore<_T>(x: number): number {
  return x;
}

class Box<T> {
  value = 1;
}

interface Tagged<T> {
  id: string;
}

type Phantom<T> = string;

declare function ambient<T>(x: number): number;

type Keys<T> = { [K in keyof T]: string };
type Unwrap<T> = T extends Promise<infer U> ? string : never;

interface Shape {
  area<T>(): number;
}

type Factory = <T>() => number;

export const exportedArrow = <T,>(x: number): number => x;

class Base {
  run<T>(x: number): number {
    return x;
  }
}

class Derived extends Base {
  override run<T>(x: number): number {
    return x;
  }
}

class Impl implements Shape {
  area<T>(): number {
    return 1;
  }
}

abstract class Abstract {
  abstract make<T>(): number;
}

class Overloads {
  private pick<T>(x: string): string;
  private pick<T>(x: number): number;
  private pick<T>(x: any): any {
    return x;
  }
}

function decorate(_target: unknown, _key: string) {}

class Decorated {
  @decorate
  private hook<T>(x: number): number {
    return x;
  }
}

[1, 2].map(<T,>(n: number) => n);

const typedHolder: (x: number) => number = <T,>(x: number) => x;

use(firstOf, wrap, Registry, used, usedInReturn, constrained, overloaded, underscore, Box);
use(Base, Derived, Impl, Abstract, Overloads, Decorated, typedHolder, ambient);
export type { Tagged, Phantom, Keys, Unwrap, Factory };

function listedLater<T>(x: number): number {
  return x;
}
const arrowListedLater = <K,>(x: number): number => x;
export { listedLater, arrowListedLater };
