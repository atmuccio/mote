export function ids(parts: string[]): number[] {
  return parts.map(parseInt); // expect: callback-extra-arguments
}

export function codes(values: number[]): string {
  return (values as number[]).map(String.fromCharCode).join(''); // expect: callback-extra-arguments
}

function scale(value: number, factor?: number): number {
  return value * (factor ?? 1);
}

export const scaled = [1, 2].map(scale);
export const fine = ['1', '2'].map((s: string) => Number.parseInt(s, 10));

declare function external(value: string, radix?: number): number;
export const ext = ['1'].map(external);
