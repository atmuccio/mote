function Input(): PropertyDecorator {
  return () => undefined;
}

export class Component {
  @Input()
  set label(value: string) {
    this._label = value.trim();
  }

  set width(value: number) { // expect: accessor-pairs
    this._width = value;
  }

  private _label = '';
  private _width = 0;
}

export abstract class Store {
  abstract set state(value: string);
}

declare class Ambient {
  set flag(value: boolean);
}

declare module 'lib' {
  class Typed {
    set flag(value: boolean);
  }
}

export class Typed {
  get count(): number {
    return this._count;
  }
  set count(value: number) {
    this._count = value;
  }
  private _count = 0;
}

export interface Shape {
  set side(value: number);
}

export const config = {
  set mode(value: 'a' | 'b') { // expect: accessor-pairs
    current = value;
  },
};
let current = 'a';
