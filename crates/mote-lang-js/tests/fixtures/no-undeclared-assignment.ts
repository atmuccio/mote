// TypeScript files are not checked: the compiler rejects undeclared names unless a declaration
// file declares them.
export function total(items: number[]) {
  sum = 0;
  for (const item of items) sum += item;
  return sum;
}
