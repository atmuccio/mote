function cached(): number {
  return (this.value = 1) as number;
}
function bare(): number {
  return this.value = 1; // expect: no-return-assign
}
const f = (x: number): number => (y = x);
const g = (x: number) => y = x as number; // expect: no-return-assign
