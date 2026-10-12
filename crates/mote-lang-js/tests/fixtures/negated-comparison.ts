export function check(value: unknown, other: string): boolean {
  return !((value as string) === other); // expect: negated-comparison
}
