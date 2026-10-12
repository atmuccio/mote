export function check(ids: string[]): boolean {
  const seen = new Set<string>(ids);
  const counts = new Map<string, number>();
  if (seen.size >= 0) use(seen); // expect: constant-length-comparison
  if (counts.size < 0) return false; // expect: constant-length-comparison
  if (0 > new Map().size) return false; // expect: constant-length-comparison
  return seen.size > -1; // expect: constant-length-comparison
}

// ---- compliant ----
export function ok(file: { size: number }, blob: Blob, ids: string[]): boolean {
  const seen = new Set(ids);
  let later = new Map();
  later = new Map([[1, 2]]);
  const Map2 = class {
    size = -1;
  };
  const custom = new Map2();
  return (
    seen.size > 0 &&
    file.size >= 0 &&
    blob.size < 0 &&
    later.size >= 0 &&
    custom.size < 0 &&
    seen?.size >= 0
  );
}

function use(x: unknown): void {
  void x;
}
