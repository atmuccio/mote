export enum Level {
  Low = 10,
  High = 20,
}

type Port = 80 | 443;
interface Box {
  kind: 3;
}

declare function Max(value: number): PropertyDecorator;

export class Settings {
  static readonly LIMIT = 10 * 1024;
  readonly retries = 3;
  @Max(100)
  size = 5;

  scale(value: number): number {
    return value * 1.25; // expect: no-magic-numbers
  }
}

export const Panel = ({ items }: { items: number[] }) => (
  <div width={300} style={{ height: 120 }} data-count={items.length > 4 ? 'many' : 'few'}> {/* expect: no-magic-numbers */}
    {items.slice(0, 3).map((i) => i * 7)} {/* expect: no-magic-numbers, no-magic-numbers */}
  </div>
);

export const typed = (100 as number) + 0;
export function f(): number {
  return (37 as number) + 1; // expect: no-magic-numbers
}
