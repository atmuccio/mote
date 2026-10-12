export class Store {
  #items = new Map<string, number>();
  private state: { mode: string; size: number } = { mode: 'a', size: 0 };

  update(size: number, mode: string): void {
    this.state.mode = mode as string; // expect: overwritten-before-read
    this.state.mode = 'c';
    this.#items.set('x', 1); // expect: overwritten-before-read
    this.#items.set('x', size satisfies number);
    this.state.size = 0 as number;
    this.state.size = size;
  }

  private flag = false;

  toggle(): void {
    this.flag = true;
    this.flag = !this.flag;
  }
}

export class Reactive {
  private _v = 0;

  get v(): number {
    return this._v;
  }

  set v(value: number) {
    this._v = value;
  }

  bump(n: number): void {
    this.v = n;
    this.v = 2;
  }
}

export function typed(target: { a?: number; b?: string }, n: number): void {
  target.a = n!; // expect: overwritten-before-read
  target.a = 2 as number;
  target.b! = 'x';
}
