export class Service {
  constructor(private readonly name: string) {}
  overload(a: string): void;
  overload(a: number): void;
  overload(a: unknown): void {
    use(a);
  }
  optionalMethod?(): void;
  dispose(): void {} // expect: no-empty-function
}
abstract class Base {
  private constructor() {}
  abstract run(): void;
  protected hook(): void {} // expect: no-empty-function
}
declare function ambient(): void;
declare module 'm' {
  function inModule(): void;
}
interface I {
  method(): void;
}
const typed: () => void = () => {}; // expect: no-empty-function
function sig(this: Window): void {} // expect: no-empty-function
export class NullLogService {
  trace(): void {}
  info(message: string): void {}
}
export const NoopTelemetry = {
  publicLog() {},
  flush: () => {},
};
const MockService = class {
  run(): void {}
};
class RealService {
  run(): void {} // expect: no-empty-function
}
