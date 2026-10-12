const wrapped = a ? (b ? 1 : 2) as number : 3; // expect: no-nested-ternary
const nonNull = a ? (b ? x : y)! : z; // expect: no-nested-ternary
type Cond<T> = T extends string ? 'a' : T extends number ? 'b' : 'c';
const single = a ? (x as number) : 0;
