// TypeScript: parameter properties, overloads, declarations.
class Base {
  constructor(protected id: string) {}
}

class Service extends Base {
  constructor(private readonly name: string) { // expect: constructor-super
    console.log(name);
  }
}

class Good extends Base {
  constructor(private readonly name: string) {
    super(name);
  }
}

class Overloaded extends Base {
  constructor(a: string);
  constructor(a: number);
  constructor(a: string | number) {
    super(String(a));
  }
}

declare class Ambient extends Base {
  constructor();
}

abstract class Shape extends Base {
  constructor() { // expect: constructor-super
    doSomething();
  }
}

declare function doSomething(): void;
export { Service, Good, Overloaded, Ambient, Shape };
