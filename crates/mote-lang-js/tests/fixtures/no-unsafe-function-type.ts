function onChange(listener: Function): void { // expect: no-unsafe-function-type
  listener();
}

let handler: Function | undefined; // expect: no-unsafe-function-type
const handlers: Function[] = []; // expect: no-unsafe-function-type
type Registry = Record<string, Function>; // expect: no-unsafe-function-type

interface Options {
  callback?: Function; // expect: no-unsafe-function-type
  make(): Function; // expect: no-unsafe-function-type
}

class Emitter {
  private listeners = new Map<string, Function>(); // expect: no-unsafe-function-type
  on(name: string, fn: Function) { // expect: no-unsafe-function-type
    this.listeners.set(name, fn);
  }
}

function isFn(value: unknown): value is Function { // expect: no-unsafe-function-type
  return typeof value === 'function';
}

const cast = handler as Function; // expect: no-unsafe-function-type

// Compliant.
function onInput(listener: (value: string) => void): void {
  listener('x');
}
type AnyFunction = (...args: never[]) => unknown;
const ctor: typeof Function = Function; // the value, not the type
const made = new Function('a', 'return a');
function wrap<T extends (...args: unknown[]) => unknown>(fn: T): T {
  return fn;
}
interface Callable {
  (): void;
}

namespace Local {
  interface Function {
    run(): void;
  }
  export const f: Function = { run() {} }; // a local interface named Function
}

declare module 'legacy-lib' {
  export function register(fn: Function): void;
}

export { onChange, handler, handlers, Registry, Options, Emitter, isFn, cast, onInput, AnyFunction, ctor, made, wrap, Callable };
