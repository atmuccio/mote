declare const ambientValue: number;
const enum Mode { A, B }
const answer: number = 42;
answer = 43; // expect: no-const-assign
export function f(): number {
  const local = Mode.A;
  return local + ambientValue;
}
