export function Row() {
  const a = 1, b = 2; // expect: one-var
  return <div>{a + b}</div>;
}
