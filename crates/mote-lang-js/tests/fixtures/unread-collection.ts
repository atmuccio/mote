interface Row {
  id?: string;
}
export function validate(rows: Row[]): boolean {
  const errors: string[] = []; // expect: unread-collection
  for (const row of rows) {
    if (!row.id) errors.push('missing id');
  }
  return true;
}
export function typed(rows: Row[]): number {
  const cache = new Map<string, Row>(); // expect: unread-collection
  type Cached = typeof cache; // a type-only use is not a read
  const view: Cached | undefined = undefined;
  rows.forEach((r) => cache.set(r.id ?? '', r));
  return view ? 1 : 0;
}
declare const ambientList: string[];
