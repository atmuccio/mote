function log(message: string): void {
  console.log(message);
}

function overloaded(x: string): void;
function overloaded(x: number): void;
function overloaded(x: unknown): void {
  console.log(x);
}

function typed(): number {
  console.log('forgot the return');
  return 1;
}

declare function external(): void;

export function positives() {
  const result = log('a'); // expect: void-result-used
  return result;
}

// ---- compliant ----
export function negatives() {
  const a = overloaded('x');
  const b = typed();
  const c = external();
  log('b');
  return [a, b, c];
}
