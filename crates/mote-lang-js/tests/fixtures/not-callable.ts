export class Service {
  run(): number {
    return 1;
  }
}

const helper = (n: number): number => n;

export function misuse(): unknown {
  const a = Service(); // expect: not-callable
  const b = new helper(1); // expect: not-callable
  return [a, b];
}

declare class Ambient {}
declare function amb(): void;
interface Callable {
  (): void;
}
export const ok = [new Service(), helper(1), (Ambient as unknown as () => void)(), amb()];
export type { Callable };
