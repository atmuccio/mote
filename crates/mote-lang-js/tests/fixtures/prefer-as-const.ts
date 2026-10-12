let mode: 'dark' = 'dark'; // expect: prefer-as-const
const size = 3 as 3; // expect: prefer-as-const
const kind = 'box' as 'box'; // expect: prefer-as-const
let quoted: "a" = 'a'; // expect: prefer-as-const
class Shape {
  type: 'circle' = 'circle'; // expect: prefer-as-const
  readonly tag: 'x' = 'x'; // expect: prefer-as-const
}

// Compliant.
let theme = 'dark' as const;
let wide: 'dark' | 'light' = 'dark';
let other: 'dark' = 'light' as any;
const n = 3 as number;
const s = 'a' as string;
let t: 'a' = `a`;
let u: 1 = 2 as 1;
let v: true = true; // expect: prefer-as-const
const w = <'w'>'w'; // expect: prefer-as-const
const w2 = <'w'>'x';
