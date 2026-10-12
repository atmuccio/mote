export function scan(tokens: string[], n: number): number {
  let count = 0;
  for (let i: number = 0; i < n; count++) { // expect: for-loop-counter-misuse
    if (tokens[i] === 'x') break;
  }
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i] === 'end') {
      i = tokens.length as number; // expect: for-loop-counter-misuse
    }
  }
  return count;
}
