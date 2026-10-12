namespace Utils { // expect: no-namespace
  export const x = 1;
}
export namespace Geometry { // expect: no-namespace
  export namespace Inner { // expect: no-namespace
    export const y = 2;
  }
}
namespace A.B.C { // expect: no-namespace
  export const z = 3;
}
module Legacy { // expect: no-namespace
  export const w = 4;
}
function merged() {}
namespace merged { // expect: no-namespace
  export const extra = true;
}

// Ambient declarations describe code that exists elsewhere.
declare module 'untyped-lib' {
  export function run(): void;
  namespace Nested {
    const v: number;
  }
}
declare global {
  interface Window { app: unknown }
}
declare namespace NodeJS {
  interface Global { x: number }
}
declare module Foo {
  export const q: number;
}
