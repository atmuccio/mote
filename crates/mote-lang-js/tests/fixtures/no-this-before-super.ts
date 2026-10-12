class Base {
  constructor(protected id: string) {}
}

class Service extends Base {
  private ready = false;
  constructor(id: string, private readonly name: string) {
    this.ready = true; // expect: no-this-before-super
    super(id);
  }
}

class Good extends Base {
  constructor(id: string, private readonly name: string) {
    super(id);
    this.log(this.name);
  }
  log(value: string): void {
    console.log(value);
  }
}

export { Service, Good };
