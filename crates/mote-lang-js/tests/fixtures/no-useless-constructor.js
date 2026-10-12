class Shape {
  constructor() {} // expect: no-useless-constructor
}
class WithParams {
  constructor(a, b) {} // expect: no-useless-constructor
}
class Circle extends Shape {
  // expect-next-line: no-useless-constructor
  constructor(...args) {
    super(...args);
  }
}
class Rect extends Shape {
  // expect-next-line: no-useless-constructor
  constructor(w, h) {
    super(w, h);
  }
}
class Args extends Shape {
  // expect-next-line: no-useless-constructor
  constructor(a) {
    super(...arguments);
  }
}
class Empty extends Shape {
  // expect-next-line: no-useless-constructor
  constructor() {
    super();
  }
}
class Square extends Shape {
  constructor(size) {
    super(size, size);
  }
}
class Reordered extends Shape {
  constructor(a, b) {
    super(b, a);
  }
}
class Fewer extends Shape {
  constructor(a, b) {
    super(a);
  }
}
class Defaults extends Shape {
  constructor(a = 1) {
    super(a);
  }
}
class Extra extends Shape {
  constructor(a) {
    super(a);
    this.init();
  }
}
class Commented {
  constructor() {
    // Instances are created by the factory only.
  }
}
class Works {
  constructor() {
    this.x = 1;
  }
}
class NoCtor {}
class Destructured extends Shape {
  constructor({ a }) {
    super({ a });
  }
}
class BaseEmptyDerived extends Shape {
  constructor() {}
}
const Expr = class extends Shape {
  // expect-next-line: no-useless-constructor
  constructor(x) {
    super(x);
  }
};
