export function positives(role: string, roles: string[], data: object) {
  const a = 'admin' in ['admin', 'owner']; // expect: in-operator-misuse
  const b = 'x' in 'xyz'; // expect: in-operator-misuse
  const c = role in `template`; // expect: in-operator-misuse
  const d = 'key' in 42; // expect: in-operator-misuse
  const e = 'admin' in roles; // expect: in-operator-misuse
  const keys = Object.keys(data);
  const f = 'name' in keys; // expect: in-operator-misuse
  const g = 'k' in null; // expect: in-operator-misuse
  return [a, b, c, d, e, f, g];
}

// ---- compliant ----
export function negatives(role: string, roles: string[], data: object, sparse: number[]) {
  const a = ['admin', 'owner'].includes(role);
  const b = 'id' in data;
  const c = 0 in sparse;
  const d = '0' in roles;
  const e = 'length' in roles;
  const f = 'flat' in [];
  const g = role in roles; // an unknown key may be an index test
  const extended: string[] = [];
  (extended as any).extra = 1;
  const h = 'extra' in extended;
  const i = 'findLast' in Array.prototype;
  return [a, b, c, d, e, f, g, h, i];
}
