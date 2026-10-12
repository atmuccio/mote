export class Dispatcher {
  private instance: Record<string, (...a: unknown[]) => void> = {};
  private ref = { instance: this.instance };

  dispatch(method: string, args: unknown[]): void {
    this.instance[method].apply(this.instance, args); // expect: prefer-spread
    this.ref.instance[method].apply(this.ref.instance, args); // expect: prefer-spread
    handlers['save'].apply(handlers, args); // expect: prefer-spread
    makeInvoker(method).apply(null, args); // expect: prefer-spread
    factory()().apply(undefined, args); // expect: prefer-spread
  }

  ok(method: string, args: unknown[], other: object): void {
    this.instance[method].apply(other, args);
    this.instance[method].apply(this, args);
    getMap()[method].apply(getMap(), args);
    makeInvoker(method).apply(this, args);
    this.instance[method].apply(this.instance, [1, 2]);
    this.instance?.[method].apply(this.instance, args); // expect: prefer-spread
  }
}

declare const handlers: Record<string, (...a: unknown[]) => void>;
declare function makeInvoker(name: string): (...a: unknown[]) => void;
declare function factory(): () => (...a: unknown[]) => void;
declare function getMap(): Record<string, (...a: unknown[]) => void>;
