export class Point {
  constructor(x, y) {
    this._x = x;
    this._y = y;
  }

  get x() {
    return this._y; // expect: accessor-field-mismatch
  }

  set x(value) {
    this._x = value;
  }

  get y() {
    return this._y;
  }

  set y(value) {
    this._x = value; // expect: accessor-field-mismatch
  }
}

// ---- compliant ----
export class Nested {
  constructor() {
    this._label = '';
    const helper = function () {
      this._text = 'another object';
    };
    helper.call({});
  }

  // `_text` is only set on another object, so no field matches `text`.
  get text() {
    return this._label;
  }

  get label() {
    return this._label;
  }
}

const plain = {
  _a: 1,
  _b: 2,
  get a() {
    return this._b;
  },
};

export { plain };
