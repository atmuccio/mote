// mote-test: params default=array readonly=generic
let a: Array<string | number>; // expect: array-type
let b: (string | number)[];
let c: readonly string[]; // expect: array-type
let d: ReadonlyArray<string>;
