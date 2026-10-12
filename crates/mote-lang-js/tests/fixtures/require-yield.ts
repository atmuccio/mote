export function* numbers(limit: number): Generator<number> { // expect: require-yield
  const out: number[] = [];
  for (let i = 0; i < limit; i++) out.push(i);
  return out.length;
}

abstract class Source {
  abstract items(): Generator<number>;
  *numbers(): Generator<number> {
    this.touch();
    return;
  }
  touch(): void {}
}
export { Source };
