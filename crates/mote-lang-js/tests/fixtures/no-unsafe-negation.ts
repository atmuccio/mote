export function missing(key: string, map: Record<string, number>): boolean {
  return !key in map; // expect: no-unsafe-negation
}
