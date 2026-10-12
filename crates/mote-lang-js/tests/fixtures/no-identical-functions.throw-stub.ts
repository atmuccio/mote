// "Not supported" stubs are meant to be repeated: a body that is a single throw is not reported.
export class ReadOnlyStore {
  set(key: string, value: string): void {
    throw new Error(
      `This store is read-only: cannot set ${key}`,
    );
  }

  delete(key: string, value: string): void {
    throw new Error(
      `This store is read-only: cannot set ${key}`,
    );
  }

  load(items: string[]) {
    let total = 0;
    for (const item of items) total += item.length;
    return total;
  }

  count(items: string[]) { // expect: no-identical-functions
    let total = 0;
    for (const item of items) total += item.length;
    return total;
  }
}
