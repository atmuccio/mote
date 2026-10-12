enum Level {
  Low = 0,
  High = 'high', // expect: no-mixed-enums
  Max = 'max',
}
enum Implicit {
  A,
  B = 'b', // expect: no-mixed-enums
}
enum StringFirst {
  A = 'a',
  B = 2, // expect: no-mixed-enums
}

// Compliant.
enum Numbers { A, B = 5, C, D = -1, E = 1 << 3 }
enum Strings { A = 'a', B = 'b', C = `c` }
enum Unknown { A = 'a', B = compute() }
enum Refs { A = 1, B = A, C = 'c' }
declare enum Ambient { A = 1, B = 'b' }
enum Empty {}
declare function compute(): number;
const enum Constants {
  Delay = 500,
  VisibleClass = 'visible',
}
