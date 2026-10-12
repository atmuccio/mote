class Cart {
  total() { return 1; }
  total() { return 2; } // expect: no-dupe-class-members

  get size() { return this._size; }
  set size(v) { this._size = v; }

  get size2() { return 1; }
  get size2() { return 2; } // expect: no-dupe-class-members

  static total() { return 3; }
  static make() {}

  count = 0;
  count() {} // expect: no-dupe-class-members

  'quoted'() {}
  quoted() {} // expect: no-dupe-class-members

  [key]() {}
  [key]() {}

  ['lit']() {}
  lit() {} // expect: no-dupe-class-members

  #secret = 1;
  secret = 2;

  get value() { return 1; }
  value() {} // expect: no-dupe-class-members

  constructor() {}
}

const Anon = class {
  run() {}
  stop() {}
  run() {} // expect: no-dupe-class-members
};

class A { m() {} }
class B { m() {} }
