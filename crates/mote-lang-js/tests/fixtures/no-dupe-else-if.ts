declare const mode: 'x' | 'y' | 'z';
export function pick(value: unknown): number {
  if (typeof value === 'string') {
    return 1;
  } else if (typeof value === 'number') {
    return 2;
  } else if (typeof value === 'string') { // expect: no-dupe-else-if
    return 3;
  } else if ((value as string) === 'a') {
    return 4;
  } else if (mode === 'x') {
    return 5;
  }
  return 0;
}
