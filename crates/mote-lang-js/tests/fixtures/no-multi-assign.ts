let p: number, q: number;
p = (q = 1) as number; // expect: no-multi-assign
p = 2;
