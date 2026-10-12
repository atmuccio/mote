export class Service {
  private cache = new Map<string, number>();
  private legacy = 'v1'; // expect: no-unused-private-class-members
  private counter = 0;
  private lookedUp = 0;
  private viaString = 0;
  private destructured = 0;
  private declare injected: string;
  @Input() private decorated = 0;
  protected shared = 0;
  public open = 0;
  private stale(): void {} // expect: no-unused-private-class-members
  private overloaded(a: string): string;
  private overloaded(a: number): number;
  private overloaded(a: any): any { return a; }
  private get label(): string { return 'x'; }
  private set target(v: number) { this.counter = v; }

  constructor(
    private readonly http: Http,
    private readonly unusedDep: Logger,
    public readonly visible: Logger,
    @Inject(TOKEN) private readonly token: string,
  ) {}

  run(other: Service): number {
    this.counter = 1;
    this.target = 5;
    const { destructured } = this;
    spyOnPrivate(this, 'viaString');
    return this.cache.size + other.lookedUp + this.http.x + this.overloaded(1) + destructured + this.label.length;
  }
}

@Component({ selector: 'app-x', templateUrl: './x.html' })
export class Widget {
  private shownInTemplate = true;
  #reallyUnused = 1; // expect: no-unused-private-class-members
}

@Injectable()
export class Api {
  constructor(private readonly client: Client) {}
  private unusedHelper(): void {} // expect: no-unused-private-class-members
}

export class Dynamic {
  private a = 1;
  private b = 2;
  get(key: 'a' | 'b') {
    return this[key];
  }
}

export class StaticInit {
  private static internalField = (function () {
    StaticInit.prototype.x = 1;
  })();
  private static arrowInit = (() => 1)();
  private static unusedCall = compute(); // expect: no-unused-private-class-members
}

export abstract class Base {
  private abstractLike = 1; // expect: no-unused-private-class-members
  protected abstract step(): void;
}

declare class Ambient {
  private hidden: number;
}

declare module 'x' {
  class InModule {
    private hidden: number;
  }
}

export class Assigner {
  private x = 0;
  private y = 0;
  constructor(init: Partial<Assigner>) {
    Object.assign(this, init);
  }
}

// Brand fields: they only make the class nominally distinct for the type checker.
export class TaggedError extends Error {
  private __brand: any;
}

export class UserId {
  private readonly _brand = 'UserId' as const;
  declare private readonly __kind: 'user';
  #brand!: never;
  constructor(readonly value: string) {}
}

export class OrderId {
  private readonly $orderBrand: void = undefined;
  private brand: unknown;
  constructor(readonly value: string) {}
}

// Not brands: a plain `brand` with a value, a name that only contains the word, or a brand field
// that is assigned but never read.
export class Product {
  private brand = 'acme'; // expect: no-unused-private-class-members
  private brandName: string = ''; // expect: no-unused-private-class-members
  #brand = 'acme'; // expect: no-unused-private-class-members
  #_brand: string | undefined; // expect: no-unused-private-class-members
  rename(name: string) {
    this.#_brand = name;
  }
}
