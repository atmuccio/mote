interface Row { id: number }
let names: Array<string>; // expect: array-type
let names2: string[];
let ids: (string | number)[]; // expect: array-type
let ids2: Array<string | number>;
let rows: readonly Row[];
let rows2: ReadonlyArray<Row>; // expect: array-type
let handlers: Array<() => void>;
let handlers2: (() => void)[]; // expect: array-type
let grid: number[][];
let grid2: Array<number[]>; // expect: array-type
let objs: { a: number }[]; // expect: array-type
let generic: Map<string, number>[]; // expect: array-type
let generic2: Array<Map<string, number>>;
let qualified: NS.Thing[];
let qualifiedGeneric: Array<NS.Thing>; // expect: array-type
let complexReadonly: readonly (string | number)[]; // expect: array-type
let nestedComplex: (string | number)[][]; // expect: array-type
let noArgs: Array<any>; // expect: array-type
let two: Array;
function f<T>(items: Array<T>): T[] { return items; } // expect: array-type
let lit: Array<'a' | 'b'>;
let thisArr: this[];
declare namespace NS { interface Thing {} }
