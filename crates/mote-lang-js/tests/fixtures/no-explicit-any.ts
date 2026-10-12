let a: any; // expect: no-explicit-any
function f(x: any): any { return x; } // expect: no-explicit-any, no-explicit-any
const y = a as any; // expect: no-explicit-any
function rest(...args: any[]) {} // expect: no-explicit-any
let u: unknown;
let s: string = 'any';
type Many = Array<any>; // expect: no-explicit-any
interface I { anyName: number; }
const g = <T,>(v: T): T => v;
