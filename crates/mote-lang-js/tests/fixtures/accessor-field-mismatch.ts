export class Box {
  private _width = 0;
  private _height = 0;

  get width(): number {
    return this._height; // expect: accessor-field-mismatch
  }

  set width(value: number) {
    this._height = value; // expect: accessor-field-mismatch
  }

  get height(): number {
    return this._height;
  }

  set height(value: number) {
    this._height = value;
  }
}

export class Private {
  #name = '';
  #title = '';

  get name(): string {
    return this.#title; // expect: accessor-field-mismatch
  }

  get title(): string {
    return this.#title;
  }
}

export class ParameterProperties {
  constructor(private readonly color_: string, private shade_: string) {}

  get color(): string {
    return this.shade_; // expect: accessor-field-mismatch
  }

  get shade(): string {
    return this.shade_;
  }
}

// ---- compliant ----
export class Derived {
  private _items: string[] = [];
  private _count = 0;

  // No field named after the accessor: nothing to compare with.
  get size(): number {
    return this._count;
  }

  get items(): string[] {
    return this._items;
  }

  // Not a plain field read.
  get count(): number {
    return this._items.length;
  }

  // More than one statement.
  set count(value: number) {
    this._count = value;
    this.refresh();
  }

  refresh(): void {}

  static _total = 0;
  static _other = 0;
  static get total(): number {
    return Derived._other;
  }
}

export class Gated {
  private _tools: string[] = [];
  private _visibleTools: string[] = [];
  private _stream = 0;
  private _routedStream = 0;

  // Exposes a derived field on purpose: no other accessor owns `_visibleTools`.
  get tools(): string[] {
    return this._visibleTools;
  }

  get stream(): number {
    return this._routedStream;
  }

  update(): void {
    this._visibleTools = this._tools.filter(Boolean);
    this._routedStream = this._stream;
  }
}

export class Alias {
  private _value = 0;

  // Reads the field under a related, prefixed name.
  get value(): number {
    return this._value;
  }

  // The setter stores a derived value, not the parameter.
  set value(input: number) {
    this._value = Math.round(input);
  }
}
