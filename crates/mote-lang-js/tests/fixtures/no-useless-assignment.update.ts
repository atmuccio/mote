export class Pager {
  private page = 0;

  next(items: string[]): string | undefined {
    let offset: number = this.page * 10;
    const item = items[offset];
    offset += 10; // expect: no-useless-assignment
    this.page++;
    return item;
  }

  typed(input: number): number {
    let n = input as number;
    type N = typeof n;
    const copy: N = n;
    n **= 2; // expect: no-useless-assignment
    return copy;
  }

  optional(list?: number[]): number {
    let total = list?.length ?? 0;
    const result = total;
    total -= 1; // expect: no-useless-assignment
    return result;
  }

  loop(list: number[]): number {
    let sum = 0;
    for (const n of list) {
      sum += n;
    }
    return sum;
  }

  counted(list: number[]): number {
    let seen = 0;
    list.forEach((n) => {
      if (n) seen++;
    });
    return seen;
  }
}

export function scoped(values: number[]): number {
  let size = 0; // expect: no-useless-assignment
  {
    const [first] = values;
    size = first ?? 0;
  }
  return size;
}
