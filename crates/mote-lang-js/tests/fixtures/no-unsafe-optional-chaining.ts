interface Group {
  members?: string[];
}
export function f(group?: Group): string[] {
  const a = [...(group?.members as string[])];
  const d = [...(group?.members satisfies string[] | undefined)]; // expect: no-unsafe-optional-chaining
  const e = (group?.members as string[]).length;
  const b = [...group?.members!];
  const c = [...(group?.members)!];
  return [...a, ...b, ...c, ...d, String(e)];
}
