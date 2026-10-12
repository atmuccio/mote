class Base {
  constructor() {} // expect: no-useless-constructor
}
class Singleton {
  private constructor() {}
}
class Guarded {
  protected constructor() {}
}
class Props {
  constructor(private readonly name: string) {}
}
class Signature {
  constructor(name: string) {}
}
class Derived extends Base {
  // expect-next-line: no-useless-constructor
  constructor() {
    super();
  }
}
class PublicDerived extends Base {
  public constructor() {
    super();
  }
}
class Narrowing extends Base {
  constructor(name: string) {
    super(name);
  }
}
class Overloads extends Base {
  constructor(a: string);
  constructor(a: number);
  constructor(a: any) {
    super(a);
  }
}
class Decorated extends Base {
  constructor(@Inject() service) {
    super(service);
  }
}
declare class Ambient {
  constructor();
}
abstract class Abstract {
  public constructor() {} // expect: no-useless-constructor
}
