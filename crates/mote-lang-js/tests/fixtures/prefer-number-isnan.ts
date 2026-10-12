export function checks(value: unknown, dim: string | number, count: number, name: string): boolean {
  if (isNaN(count)) {
    return false;
  }
  if (isNaN(value as number)) {
    return false;
  }
  if (!isNaN(dim as any)) { // expect: prefer-number-isnan
    return true;
  }
  if (isNaN(<any>value)) { // expect: prefer-number-isnan
    return true;
  }
  if (isNaN(name as unknown as number)) { // expect: prefer-number-isnan
    return true;
  }
  if (isFinite(value as string)) { // expect: prefer-number-isnan
    return true;
  }
  const label = 'abc';
  if (isNaN(label as any)) { // expect: prefer-number-isnan
    return true;
  }
  return isNaN(compute()) || isFinite(dim as number);
}
declare function compute(): number;
