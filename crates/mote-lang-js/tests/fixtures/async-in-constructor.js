class Catalog {
  constructor(api) {
    this.api = api;
    this.load(); // expect: async-in-constructor
    this.refresh(1); // expect: async-in-constructor
    fetch('/x').then((r) => { this.status = r.status; }); // expect: async-in-constructor
    (async () => { // expect: async-in-constructor
      this.tags = await api.tags();
    })();
  }
  async load() {
    this.items = await this.api.items();
  }
  refresh = async (n) => {
    await wait(n);
    this.version = n;
  };
  get count() {
    return this.items.length + this.version + this.status + this.tags.length;
  }
}

class Compliant {
  constructor(api) {
    this.ready = this.load();
    void this.load();
    this.load().catch((e) => console.error(e));
    this.load().then(done, fail);
    this.sync();
    this.el.addEventListener('click', () => this.load());
    setTimeout(() => this.load(), 10);
    const p = this.load();
    this.pending = p;
    Other.load();
    this.report();
    this.prepare();
    fetch('/y').then((r) => log(r));
    fetch('/z').then(function (r) { this.value = r; });
    this.unreadField();
  }
  async load() {
    this.data = await fetch('/data');
  }
  async report() {
    await telemetry.send('opened');
  }
  async prepare() {
    this.early = 1;
    await other();
  }
  async unreadField() {
    this.neverRead = await fetch('/z');
  }
  sync() {}
  use() {
    return this.data + this.early + this.value;
  }
}

class Sub extends Base {
  constructor() {
    super();
    this.init(); // not async in this class (the parent is unknown)
  }
  use() {
    return this.state;
  }
}
