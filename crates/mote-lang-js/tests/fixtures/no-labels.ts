export function scan(grid: number[][]): number {
  rows: for (const row of grid) { // expect: no-labels
    for (const v of row) {
      if (v < 0) continue rows;
    }
  }
  return 0;
}
type Labelled = { label: string };
