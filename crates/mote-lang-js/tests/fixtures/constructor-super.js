// Derived classes whose constructor never calls super().
class Base {}

class Admin extends Base {
  constructor(name) { // expect: constructor-super
    this.name = name;
  }
}

const Mixed = class extends Base {
  constructor() { // expect: constructor-super
    setup();
  }
};

class Outer extends Base {
  constructor() { // expect: constructor-super
    // The super() below belongs to the nested class, not to this constructor.
    this.inner = class extends Base {
      constructor() {
        super();
      }
    };
  }
}

// Compliant.
class Ok extends Base {
  constructor(name) {
    super(name);
    this.name = name;
  }
}

class Conditional extends Base {
  constructor(flag) {
    if (flag) {
      super(1);
    } else {
      super(2);
    }
  }
}

class ViaArrow extends Base {
  constructor() {
    const init = () => super();
    init();
  }
}

class ReturnsObject extends Base {
  constructor() {
    return Object.create(null);
  }
}

class NullBase extends null {
  constructor() {
    return Object.create(NullBase.prototype);
  }
}

class NoBase {
  constructor() {
    this.x = 1;
  }
}

class Implicit extends Base {}

class Other extends Base {
  method() {
    return 1;
  }
}

export { Admin, Mixed, Outer, Ok, Conditional, ViaArrow, ReturnsObject, NullBase, NoBase, Implicit, Other };
