export function first(items: string[]): string | undefined {
  let item: string | undefined;
  while ((item = items.shift()) !== undefined) {
    if (item = item.trim()) { // expect: no-cond-assign
      return item;
    }
  }
  return undefined;
}
