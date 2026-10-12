function greet(name: String): Boolean { return !!name; } // expect: no-wrapper-object-types, no-wrapper-object-types
let options: Object = {}; // expect: no-wrapper-object-types
let count: Number; // expect: no-wrapper-object-types
let big: BigInt; // expect: no-wrapper-object-types
let sym: Symbol; // expect: no-wrapper-object-types
let list: Array<String> = []; // expect: no-wrapper-object-types
class Wrapped implements Object {} // expect: no-wrapper-object-types
interface Branded extends String {} // expect: no-wrapper-object-types

// Compliant: primitives, constructors as values, qualified or local names.
function ok(name: string): boolean { return !!name; }
let o: object = {};
let u: unknown;
const made = String(1);
const ctor: typeof String = String;
let qualified: globalThis.String;
let ns: Lib.Object;
declare namespace Lib { interface Object { x: number } }
