export class Store {
  constructor(private readonly name: string, size: number) {
    size = Math.max(size, 1); // expect: no-param-reassign
    this.size = size;
  }
  overload(a: string): void;
  overload(a: number): void;
  overload(a: string | number): void {
    a = String(a); // expect: no-param-reassign
    use(a);
  }
  narrow(value: unknown): void {
    (value as any).x = 1;
    use(value!);
  }
}
declare function external(x: number): void;
type Fn = (x: number) => void;
