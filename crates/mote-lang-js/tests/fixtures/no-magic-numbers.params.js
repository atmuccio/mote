// mote-test: params ignore=0,1,100
export function percent(part, total) {
  return (part / total) * 100;
}
export function twice(x) {
  return x * 2; // expect: no-magic-numbers
}
