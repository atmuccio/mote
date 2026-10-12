function first<T extends unknown>(items: T[]): T { return items[0]; } // expect: no-unnecessary-type-constraint
function any<T extends any>(x: T) { return x; } // expect: no-unnecessary-type-constraint
const arrow = <T extends unknown>(x: T) => x; // expect: no-unnecessary-type-constraint
class Box<T extends unknown, U extends any> {} // expect: no-unnecessary-type-constraint, no-unnecessary-type-constraint
type Alias<T extends unknown> = T[]; // expect: no-unnecessary-type-constraint
interface Holder<
  T extends unknown, // expect: no-unnecessary-type-constraint
> { value: T }

// Compliant.
function plain<T>(x: T) { return x; }
function constrained<T extends { id: string }>(x: T) { return x.id; }
function keyed<K extends keyof any>(k: K) { return k; }
function arrays<T extends unknown[]>(x: T) { return x; }
function withDefault<T = unknown>(x: T) { return x; }
type Cond<T> = T extends unknown ? T : never;
