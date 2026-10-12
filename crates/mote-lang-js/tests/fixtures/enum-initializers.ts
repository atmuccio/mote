export const enum State {
  Idle, // expect: enum-initializers
  Busy,
  Done = 2,
}

export enum Level {
  Low = 1,
  Medium, // expect: enum-initializers
  High = 2,
}

export enum Wrap {
  A = -1,
  B = 0,
  C, // expect: enum-initializers
  D = 1,
}

// Compliant.
export enum Direction { Up, Down, Left, Right }
export enum Code { A = 100, B, C }
export enum Explicit { A = 1, B = 2, C = 4 }
export enum Strings { A = 'a', B = 'b' }
export enum Empty {}
export enum Order {
  // Navigation
  Refresh = 10,
  Run,
  Debug,
  // Submenu
  Collapse = 20,
  Clear,
}
export enum Key { DependsOnLayout = -1, Unknown = 0, Backspace, Tab }
export enum Computed { A = 1 << 0, B = 1 << 1, C, D = 8 }
declare enum Ambient { A = 1, B }
