function defaultLater<T>(x: number): number {
  return x;
}
function stillPrivate<U>(x: number): number { // expect: no-unused-vars
  return x;
}
export default defaultLater;
export const used = stillPrivate(1);
