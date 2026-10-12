export class Service {
  stop(): void {
    this.running = false;
    return; // expect: no-useless-return
  }
  value(): number | undefined {
    return;
    this.unreachable();
  }
  declare overload: () => void;
}
export async function maybe(flag: boolean): Promise<number | undefined> {
  if (flag) {
    const inner = () => {
      return;
    };
    inner();
    return 1;
  }
  return;
}
export function nestedValue(): void {
  const f = () => 1;
  const g = function () {
    return 2;
  };
  use(f, g);
  return; // expect: no-useless-return
}
export function drain(queue: Array<string | undefined>): void {
  for (const entry of queue) {
    const value: string = entry ?? '';
    consume(value);
    continue; // expect: no-useless-return
  }
}
