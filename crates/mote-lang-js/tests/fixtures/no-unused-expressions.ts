declare const value: string | undefined;
value!; // expect: no-unused-expressions
value as string; // expect: no-unused-expressions
(value as unknown as () => void)();
export namespace N {
  'use strict';
  export const x = 1;
}
enum E { A }
E.A;
export {};
