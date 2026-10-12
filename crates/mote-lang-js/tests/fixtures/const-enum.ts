export const enum Direction { Up, Down } // expect: const-enum
const enum Local { A = 1 } // expect: const-enum
enum Regular { A }
declare const enum Ambient { A }
declare module 'x' {
  const enum InModule { A }
}
declare global {
  const enum InGlobal { A }
}
