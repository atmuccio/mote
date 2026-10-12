export function parse(input: string): number;
export function format(value: number): string;
export function parse(input: number): number; // expect: adjacent-overload-signatures
export function parse(input: any): number { return 0; }
export function format(value: any): string { return ''; } // expect: adjacent-overload-signatures

declare function load(a: string): void;
declare const marker: number;
declare function load(a: number): void; // expect: adjacent-overload-signatures

interface Emitter {
  on(event: 'open', cb: () => void): void;
  close(): void;
  on(event: 'data', cb: (d: string) => void): void; // expect: adjacent-overload-signatures
  (x: number): void;
  name: string;
  (x: string): void; // expect: adjacent-overload-signatures
  new (x: number): Emitter;
  off(): void;
  new (x: string): Emitter; // expect: adjacent-overload-signatures
}

type Handler = {
  handle(x: number): void;
  size: number;
  handle(x: string): void; // expect: adjacent-overload-signatures
};

class Service {
  constructor(a: string);
  constructor(a: number);
  constructor(a: any) {}
  run(a: string): void;
  stop(): void {}
  run(a: number): void; // expect: adjacent-overload-signatures
  run(a: any): void {}
  static create(): Service;
  create(): void {}
  static create(x?: number): Service { return new Service(1); } // expect: adjacent-overload-signatures
  #secret(): void {}
  secret(): void {}
}

namespace Api {
  export function get(url: string): void;
  export const version = 1;
  export function get(url: URL): void; // expect: adjacent-overload-signatures
  export function get(url: any): void {}
}

// Compliant.
function ok(a: string): void;
function ok(a: number): void;
function ok(a: any): void {}
interface Fine {
  on(event: 'open'): void;
  on(event: 'close'): void;
  off(): void;
}
class Accessors {
  get value(): number { return 1; }
  other(): void {}
  set value(v: number) {}
  static make(): void {}
  make(): void {}
  instance(): void {}
}
class Separate {
  a(): void {}
  b(): void {}
}
function outer() {
  function inner(): void {}
  const x = 1;
  return x;
}
