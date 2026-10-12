export class A {
  dispose(): void {
    this.listeners.clear();
    this.cache.clear();
    this.disposed = true;
  }
  overload(a: string): void;
  overload(a: number): void;
  overload(a: unknown): void {
    use(a);
    use(a);
    use(a);
  }
}
export class B {
  dispose(): void { // expect: no-identical-functions
    this.listeners.clear();
    this.cache.clear();
    this.disposed = true;
  }
  typed(): number {
    const v = this.value as number;
    use(v);
    return v;
  }
  typed2(): number {
    const v = this.value as string;
    use(v);
    return v;
  }
}
declare function ambient(): void;
