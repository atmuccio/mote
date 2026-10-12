export function clean(name: string, list: number[], tags: ReadonlyArray<string>, raw: Array<number>): void {
  name.trim(); // expect: ignored-call-result
  name.trim().toLowerCase(); // expect: ignored-call-result
  list.concat([4]); // expect: ignored-call-result
  tags.slice(1); // expect: ignored-call-result
  raw.toSorted(); // expect: ignored-call-result
  name.replace(/x/g, ''); // expect: ignored-call-result
  'abc'.toUpperCase(); // expect: ignored-call-result
  String(list).padStart(4); // expect: ignored-call-result
  const greeting = 'hello';
  greeting.concat(' world'); // expect: ignored-call-result
  const items = [1, 2];
  items.join(','); // expect: ignored-call-result
  `x${name}`.trim(); // expect: ignored-call-result
  name?.trim(); // expect: ignored-call-result
}

// ---- compliant ----
export function ok(name: string, list: number[], other: Thing, value: string | number): string {
  name = name.trim();
  list.push(4);
  list.sort();
  list.map((n) => console.log(n));
  name.replace(/x/g, (m) => {
    found.push(m);
    return m;
  });
  other.trim();
  value.toString();
  const literal = [1];
  literal.concat([2]); // expect: ignored-call-result
  let changed = 'a';
  changed = 'b';
  changed.trim();
  const map = (s: string) => s.trim();
  list.forEach((n) => n);
  return map(name);
}

interface Thing {
  trim(): void;
}
declare const found: string[];
