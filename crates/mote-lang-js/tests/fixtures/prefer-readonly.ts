import { Injectable, Input, Inject, ViewChild } from '@angular/core';

export class Cache {
  private store = new Map<string, number>(); // expect: prefer-readonly
  private hits = 0;
  #secret = 'x'; // expect: prefer-readonly
  #counter = 0;
  private late: string; // expect: prefer-readonly
  private reset = 1;

  constructor(private http: HttpClient, private readonly log: Logger, public open: boolean) { // expect: prefer-readonly
    this.late = 'set in constructor';
    this.reset = 2;
  }

  get(key: string): number | undefined {
    this.hits++;
    this.#counter += 1;
    return this.store.get(key) ?? this.http.get(key) ?? this.#secret.length;
  }

  clear(): void {
    this.reset = 0;
    this.store.clear();
  }
}

@Injectable()
export class Service {
  @Input() private label = '';
  @ViewChild('ref') private ref: unknown;

  constructor(@Inject(TOKEN) private token: string, private api: Api) { // expect: prefer-readonly
    use(this.label, this.ref, this.token);
  }
}

// ---- compliant ----

export class Mutations {
  private a = 1;
  private b = 2;
  private c = 3;
  private d = 4;
  private e = 5;
  private f: number[] = []; // expect: prefer-readonly
  private g = 7;
  private h?: number;
  private static count = 0;
  private readonly fixed = 1;
  protected shared = 1;
  public open = 1;
  private arrowSet = 0;
  #viaOther = 0;

  constructor() {
    setTimeout(() => {
      this.arrowSet = 1;
    });
  }

  run(other: Mutations): void {
    [this.a, this.b] = [this.b, this.a];
    ({ x: this.c } = { x: 1 });
    for (this.d of [1, 2]) use(this.d);
    this.e *= 2;
    (this.g as number) = 3;
    delete this.h;
    other.#viaOther = 1;
    Mutations.count++;
    use(this.f, this.fixed, this.shared, this.open);
  }

  get value(): number {
    return this.accessorBacked;
  }
  private accessorBacked = 1;
  set value(v: number) {
    this.accessorBacked = v;
  }
}

export class Dynamic {
  private x = 1;
  private y = 2;

  apply(data: object, key: string): void {
    Object.assign(this, data);
    (this as any)[key] = 1;
  }
}

export class ExternalWrite {
  private z = 1;
}
const instance = new ExternalWrite();
(instance as any)['z'] = 2;

export class StaticBlock {
  private w = 1;
  static make(): StaticBlock {
    const s = new StaticBlock();
    s.w = 2;
    return s;
  }
}

declare class Ambient {
  private q: number;
}

export abstract class Base {
  private abstract_like = 1;
  protected constructor() {}
  bump(): void {
    this.abstract_like = 2;
  }
}
