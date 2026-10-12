import type { OnlyType } from './types';
import { UnusedType } from './types'; // expect: no-unused-vars
import { ValueUsedAsType, type Inline } from './types';
import Default, { type Unused2 } from './other'; // expect: no-unused-vars
import fs = require('fs'); // expect: no-unused-vars

export function f(x: OnlyType, y: Inline): ValueUsedAsType { // expect: no-unused-vars
  return x as unknown as typeof Default;
}

interface UnusedInterface { a: number } // expect: no-unused-vars
type UnusedAlias = string; // expect: no-unused-vars
interface UsedInterface { b: number }
export const v: UsedInterface = { b: 1 };
type Recursive = { next: Recursive }; // expect: no-unused-vars

enum UnusedEnum { A } // expect: no-unused-vars
enum UsedEnum { B }
export const e = UsedEnum.B;
const enum ConstEnum { C } // expect: no-unused-vars

declare const ambient: number;
declare function ambientFn(): void;
declare module 'augment' {
  const inside: number;
  function helper(): void;
}
declare global {
  interface Window { custom: string }
}

interface Merged { a: number }
class Merged {}

function overloaded(a: string): void;
function overloaded(a: number): void;
function overloaded(a: unknown): void {} // expect: no-unused-vars

namespace Space {
  const hidden = 1; // expect: no-unused-vars
  export const visible = 2;
}

export class Component {
  constructor(private readonly service: Service, public name: string, unused: number) {} // expect: no-unused-vars
  method(this: Component, value: number): void { this.name = ''; } // expect: no-unused-vars
  onChange(value: number): void {} // an empty hook for subclasses
  overload(a: string): void;
  overload(a: unknown): void { void a; }
  withCallback(cb: (err: Error, data: string) => void) { return cb; }
  set size(value: number) {}
  abstractLike(@Inject() token: string) {}
}

export abstract class Base {
  abstract run(input: string): void;
}

export function generic<T, U>(value: T): T {
  using handle = open();
  return value;
}

export type Fn = (a: string, b: number) => void;
export interface Api { call(a: string): void }
export default function main(this: Window, event: Event): void {} // expect: no-unused-vars

import { DocLinked, DocTyped, NotDocumented } from './docs'; // expect: no-unused-vars
/**
 * See {@link DocLinked} for details.
 * @param value {DocTyped} typed in the docs only
 */
export function documented(value: unknown): void { void value; }

export class Impl implements Api {
  call(a: string): void {}
}

export class Child extends Base {
  run(input: string): void {}
  override toString(radix: number): string { return ''; }
}

export const handler: Fn = (a, b) => {};
export const provider = { provide(document: string) {} };
export function assertNever(value: never): never { throw new Error('unreachable'); }
