export const cfg: Record<string, number> = { a: 1, b: 2, a: 3 }; // expect: no-dupe-keys
enum E { A, B }
export const byEnum = { [E.A]: 1, [E.B]: 2 };
export const typed = { x: 1 } satisfies { x: number };
