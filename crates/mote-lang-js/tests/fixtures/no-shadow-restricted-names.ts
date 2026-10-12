declare var NaN: number;
declare function eval(code: string): unknown;
declare global {
  var undefinedValues: number;
}
declare module 'numbers' {
  export const undefined: unknown;
}

enum Special {
  NaN,
  Infinity,
}
interface Infinity2 {}
type undefinedLike = undefined;

function typed(value: number, NaN: boolean) { // expect: no-shadow-restricted-names
  return NaN ? value : 0;
}

export class Box {
  constructor(private readonly Infinity: string) {} // expect: no-shadow-restricted-names
}

import { undefined as nothing } from './values';
import { Infinity } from './limits'; // expect: no-shadow-restricted-names

// Parameters of type signatures and overloads only describe types.
type Callback = (undefined: number, NaN: string) => void;
interface Parser {
  parse(Infinity: number): void;
  new (NaN: number): Parser;
}
abstract class Shape {
  abstract area(NaN: number): number;
}
function overloaded(NaN: number): void;
function overloaded(value: number): void {
  return;
}
