let count: number = 0; // expect: no-inferrable-types
const label: string = 'x'; // expect: no-inferrable-types
let greeting: string = `hi ${label}`; // expect: no-inferrable-types
let enabled: boolean = false; // expect: no-inferrable-types
let negated: boolean = !enabled; // expect: no-inferrable-types
let big: bigint = 10n; // expect: no-inferrable-types
let negative: number = -1; // expect: no-inferrable-types
let infinite: number = Infinity; // expect: no-inferrable-types
let sym: symbol = Symbol('s'); // expect: no-inferrable-types
let re: RegExp = /ab+c/; // expect: no-inferrable-types

function greet(name: string = 'world', times: number = 1) {} // expect: no-inferrable-types, no-inferrable-types
const arrow = (flag: boolean = true) => flag; // expect: no-inferrable-types

class Timer {
  private running: boolean = false; // expect: no-inferrable-types
  static retries: number = 3; // expect: no-inferrable-types
  readonly limit: number = 5;
  optional?: number = 5;
  declare other: number;
  constructor(private delay: number = 10) {} // expect: no-inferrable-types
  plain = 0;
}

// Compliant: no annotation, wider or different types, non-literal initializers.
let a = 0;
let b: number | undefined = 0;
let c: 'fast' | 'slow' = 'fast';
let d: number = compute();
let e: string = String(1);
let f: unknown = 1;
let g: any = 'x';
let h: null = null;
let i: undefined = undefined;
let j: Number = 1;
let k: object = {};
let l: number;
let m: string = 1 as any;
let { n }: { n: number } = { n: 1 };
function p(q: number, r = 2) {}
declare function compute(): number;
function shadowed() {
  const Infinity = 5;
  let s: number = Infinity;
}
