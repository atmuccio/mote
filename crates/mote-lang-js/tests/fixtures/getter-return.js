class Order {
  get total() { // expect: getter-return
    this.items.reduce((sum, i) => sum + i.price, 0);
  }
  get count() {
    return this.items.length;
  }
  get empty() {} // expect: getter-return
  get bare() { // expect: getter-return
    return;
  }
  get thrower() {
    throw new Error('abstract');
  }
  get conditional() {
    if (this.x) {
      return 1;
    }
  }
  set total(v) {}
  method() {}
  static get instance() { // expect: getter-return
    Order._instance = Order._instance || new Order();
  }
  get nested() { // expect: getter-return
    const f = () => {
      return 1;
    };
    f();
  }
}
const obj = {
  get name() { // expect: getter-return
    this._name;
  },
  get ok() {
    return 1;
  },
  set name(v) {},
};
Object.defineProperty(obj, 'x', {
  get() { // expect: getter-return
    compute();
  },
});
Object.defineProperty(obj, 'y', {
  get: function () { // expect: getter-return
    compute();
  },
  enumerable: true,
});
Object.defineProperty(obj, 'z', {
  get() {
    return compute();
  },
});
Object.defineProperties(obj, {
  w: {
    get() { // expect: getter-return
      compute();
    },
  },
});
const notDescriptor = { get() { compute(); } };
someCall({ get() { compute(); } });
