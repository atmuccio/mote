// `this` / `super.x` used before super() in a derived constructor.
class Base {
  init() {}
}

class Button extends Base {
  constructor(label) {
    this.label = label; // expect: no-this-before-super
    super(label);
  }
}

class Args extends Base {
  constructor() {
    super(this.defaults()); // expect: no-this-before-super
  }
}

class Member extends Base {
  constructor() {
    super.init(); // expect: no-this-before-super
    super();
  }
}

class Branch extends Base {
  constructor(flag) {
    if (flag) {
      this.flag = flag; // expect: no-this-before-super
    }
    super();
  }
}

// Compliant.
class Ok extends Base {
  constructor(label) {
    super(label);
    this.label = label;
    super.init();
  }
}

class LaterArrow extends Base {
  constructor() {
    const read = () => this.value;
    super();
    this.value = read();
  }
}

class NestedFunction extends Base {
  constructor() {
    const self = {
      run: function () {
        return this;
      },
    };
    super(self);
  }
}

class NestedClass extends Base {
  constructor() {
    const Inner = class {
      field = this;
    };
    super(Inner);
  }
}

class NoBase {
  constructor() {
    this.x = 1;
  }
}

class NoSuperAtAll extends Base {
  constructor() {
    return { value: 1 };
  }
}

export { Button, Args, Member, Branch, Ok, LaterArrow, NestedFunction, NestedClass, NoBase, NoSuperAtAll };
