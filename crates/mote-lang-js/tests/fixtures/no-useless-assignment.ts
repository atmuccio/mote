export class Service {
  private cache = new Map<string, number>();

  lookup(key: string): number {
    let value: number | undefined = this.cache.get(key); // expect: no-useless-assignment
    value = compute(key);
    return value;
  }

  async load(id: string): Promise<string> {
    let text: string = ''; // expect: no-useless-assignment
    text = await fetchText(id);
    if (!text) {
      text = 'missing'; // expect: no-useless-assignment
      throw new Error('not found');
    }
    return text;
  }

  *items(list: number[]) {
    let last = 0;
    for (const n of list) {
      yield last;
      last = n;
    }
  }

  get size(): number {
    let n = 0;
    this.cache.forEach(() => {
      n++;
    });
    return n;
  }

  typed(input: unknown): string {
    let s = input as string;
    s = s.trim();
    return s;
  }

  typeOnly(): void {
    let shape = { a: 1 };
    type Shape = typeof shape;
    const copy: Shape = { a: 2 };
    shape = copy; // expect: no-useless-assignment
  }
}

export function generic<T>(items: T[]): T | undefined {
  let found: T | undefined;
  found = items[0];
  for (const item of items) {
    if (item) {
      found = item;
      break;
    }
  }
  return found;
}

export function switchDefault(kind: number): string {
  let result = '';
  switch (kind) {
    case 0: {
      result = 'zero';
      break;
    }
  }
  return result;
}

export function staticBlockHolder() {
  return class {
    static value: number;
    static {
      let v = 1;
      v = 2;
      this.value = v;
    }
  };
}

declare function compute(key: string): number;
declare function fetchText(id: string): Promise<string>;
