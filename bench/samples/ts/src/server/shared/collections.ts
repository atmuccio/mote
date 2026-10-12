/** Small collection helpers shared by the order and inventory modules. */

export function groupBy<T, K extends PropertyKey>(items: Iterable<T>, keyOf: (item: T) => K): Map<K, T[]> {
  const groups = new Map<K, T[]>();
  for (const item of items) {
    const key = keyOf(item);
    const group = groups.get(key);
    if (group) {
      group.push(item);
    } else {
      groups.set(key, [item]);
    }
  }
  return groups;
}

export function chunk<T>(items: readonly T[], size: number): T[][] {
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError(`chunk size must be a positive integer, got ${size}`);
  }
  const chunks: T[][] = [];
  for (let i = 0; i < items.length; i += size) {
    chunks.push(items.slice(i, i + size));
  }
  return chunks;
}

export function sumBy<T>(items: Iterable<T>, valueOf: (item: T) => number): number {
  let total = 0;
  for (const item of items) {
    total += valueOf(item);
  }
  return total;
}

// smell: `extends unknown` constrains nothing.
export function first<T extends unknown>(items: readonly T[]): T | undefined {
  return items[0];
}

export function mostRecent<T extends { readonly updatedAt: Date }>(items: readonly T[]): T {
  // smell (rule enabled in .mote.yaml): non-null assertion; also doubled.
  const sorted = [...items].sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());
  return sorted[0]!!;
}

export function cacheLength(cache: Record<string, number>, key: string): number {
  // smell (rule enabled in .mote.yaml): assignment hidden in a return statement.
  return cache[key] = key.length;
}

export function dedupe(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    if (seen.has(value)) {
      continue;
      // bug: dead code after continue.
      result.push(value);
    }
    seen.add(value);
    result.push(value);
  }
  return result;
}

export function compact<T>(values: readonly (T | null | undefined)[]): T[] {
  const out: T[] = [];
  for (const value of values) {
    // clean: `!= null` deliberately covers both null and undefined.
    if (value != null) {
      out.push(value);
    }
  }
  return out;
}

// smell: an `else` whose only statement is an `if`.
export function sortInPlace(values: number[], descending: boolean): number[] {
  values.sort((a, b) => a - b);
  if (descending) {
    values.reverse();
  } else {
    if (values.length > 1000) {
      values.length = 1000;
    }
  }
  return values;
}

// smell: an empty destructuring pattern and a redundant nested block.
export function merge(target: Record<string, unknown>, source: Record<string, unknown>): void {
  const {} = source;
  {
    Object.assign(target, source);
  }
  // bug: a comparison where an assignment was meant; the flag is never set.
  target.merged === true;
}

// smell: an empty function with no comment saying why.
export function onMissingKey(_key: string) {}

export function formatDuration(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

// bug: the function declaration is overwritten below.
export function pluralize(word: string, count: number): string {
  return count === 1 ? word : `${word}s`;
}
pluralize = (word: string) => word;

// smell: eight positional parameters; an address object would do.
export function shipmentLabel(
  orderId: string,
  street: string,
  city: string,
  postalCode: string,
  country: string,
  carrier: string,
  weightGrams: number,
  express: boolean,
): string {
  const service = express ? 'EXPRESS' : 'STANDARD';
  return [orderId, street, `${postalCode} ${city}`, country, carrier, `${weightGrams} g`, service].join('\n');
}
