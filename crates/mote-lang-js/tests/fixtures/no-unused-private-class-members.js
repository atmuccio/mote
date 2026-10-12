class Counter {
  #count = 0;
  #unused = 1; // expect: no-unused-private-class-members
  #writeOnly; // expect: no-unused-private-class-members
  static #registry = new Map();
  #helper() { return 1; } // expect: no-unused-private-class-members
  #used() { return this.#count; }
  get #doubled() { return this.#count * 2; }
  set #value(v) { this.#count = v; }
  #brand = true;

  increment() {
    this.#count++;
    this.#writeOnly = Date.now();
    this.#value = 3;
    return this.#used() + this.#doubled;
  }

  static register(c) {
    Counter.#registry.set(c, true);
  }

  static isCounter(o) {
    return #brand in o;
  }
}

class Other {
  #a = 0;
  #b = 0;
  #c = 0;
  #d = 0;
  #e = 0; // expect: no-unused-private-class-members

  run(list) {
    const fn = () => this.#a;
    list.forEach(function () { return this; });
    this.#b += 1;
    this.#c ??= 2;
    [this.#e] = list;
    return fn() + inner(this);
    function inner(self) { return self.#d; }
  }
}

class Outer {
  #shared = 1;
  make() {
    return class Inner {
      read(o) { return o.#shared; }
    };
  }
}

const Expr = class {
  #x = 1; // expect: no-unused-private-class-members
  public() {}
};
