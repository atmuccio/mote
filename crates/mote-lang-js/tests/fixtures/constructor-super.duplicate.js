class Admin extends User {
  constructor(name) {
    super(name);
    this.role = 'admin';
    super(name); // expect: constructor-super
  }
}

class Nested extends Base {
  constructor() {
    {
      super();
    }
    super(); // expect: constructor-super
  }
}

// ---- compliant ----

class Branches extends Base {
  constructor(a) {
    if (a) {
      super(1);
    } else {
      super(2);
    }
  }
}

class EarlyReturn extends Base {
  constructor(a) {
    if (a) {
      super(1);
      return;
    }
    super(2);
  }
}

class Callback extends Base {
  constructor() {
    const init = () => super();
    super.method?.();
    init();
  }
}

class Once extends Base {
  constructor() {
    super();
    this.x = 1;
  }
}

class Tried extends Base {
  constructor() {
    try {
      super();
    } catch (e) {
      super(e);
    }
  }
}
