@Component({ selector: 'app-list' })
export class ListComponent {
  items!: Item[];
  constructor(private readonly service: ItemService) {
    this.loadItems(); // expect: async-in-constructor
  }
  private async loadItems(): Promise<void> {
    this.items = await this.service.list();
  }
  get empty(): boolean {
    return this.items.length === 0;
  }
}

export class Loader {
  readonly ready: Promise<void>;
  private value?: string;
  constructor() {
    this.ready = this.init();
  }
  private async init(): Promise<void> {
    this.value = await read();
  }
  get current() {
    return this.value;
  }
}

export class Prepared {
  private ready = false;
  private data?: Data;
  private other: Data | undefined;
  private assigned: Data;
  constructor() {
    this.assigned = empty();
    this.start();
  }
  private async start(): Promise<void> {
    this.ready = await check();
    this.data = await load();
    this.other = await load();
    this.assigned = await load();
  }
  get state() {
    return [this.ready, this.data, this.other, this.assigned];
  }
}
