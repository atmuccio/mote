class WriteOnly {
  set size(n) { // expect: accessor-pairs
    this._size = n;
  }
}

class Paired {
  get size() {
    return this._size;
  }
  set size(n) {
    this._size = n;
  }
}

class GetterOnly {
  get area() {
    return 1;
  }
}

class StaticMismatch {
  static get mode() {
    return 'a';
  }
  set mode(m) { // expect: accessor-pairs
    this._mode = m;
  }
}

class StaticPaired {
  static get level() {
    return StaticPaired._level;
  }
  static set level(l) {
    StaticPaired._level = l;
  }
}

class Private {
  set #secret(v) { // expect: accessor-pairs
    this.value = v;
  }
  get secret() {
    return 1;
  }
  get #other() {
    return 2;
  }
  set #other(v) {
    this.o = v;
  }
}

class Base {
  get value() {
    return this._v;
  }
  set value(v) {
    this._v = v;
  }
}
class Derived extends Base {
  set value(v) { // expect: accessor-pairs
    super.value = v * 2;
  }
}

class Computed {
  set [key](v) {
    this.k = v;
  }
  set ['literal'](v) { // expect: accessor-pairs
    this.l = v;
  }
}

class WithField {
  name = 'x';
  set name(v) {
    this._name = v;
  }
}

const literal = {
  set total(v) { // expect: accessor-pairs
    this._total = v;
  },
};

const literalPaired = {
  get total() {
    return this._total;
  },
  set total(v) {
    this._total = v;
  },
};

const withSpread = {
  ...base,
  set total(v) {
    this._total = v;
  },
};

const vueComputed = {
  full: {
    get() {
      return 'a';
    },
    set(v) {
      this.v = v;
    },
  },
  writeOnly: {
    set(v) {
      this.v = v;
    },
  },
};

const handler = new Proxy(target, {
  set(obj, prop, value) {
    obj[prop] = value;
    return true;
  },
});

Object.defineProperty(obj, 'x', {
  set: function (v) { // expect: accessor-pairs
    this._x = v;
  },
});

Object.defineProperty(obj, 'y', {
  get() {
    return this._y;
  },
  set(v) {
    this._y = v;
  },
});

Reflect.defineProperty(obj, 'z', { set(v) { this._z = v; } }); // expect: accessor-pairs

Object.defineProperties(obj, {
  a: { set(v) { this._a = v; } }, // expect: accessor-pairs
  b: { get() { return 1; }, set(v) { this._b = v; } },
  c: { value: 3 },
});

Object.defineProperty(obj, 'w', { ...common, set(v) { this._w = v; } });
Object.defineProperty(obj, 'v', { value: 1, writable: true });

function shadowed(Object) {
  Object.defineProperty(obj, 'q', { set(v) { this._q = v; } });
}

class Stub {
  set ignored(value) {}
}
const stubLiteral = {
  set ignored(value) {},
};
