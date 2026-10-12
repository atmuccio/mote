function find(rows, query) {
  let found = null;
  search: // expect: no-labels
  for (const row of rows) {
    for (const cell of row) {
      if (cell === query) {
        found = cell;
        break search;
      }
    }
  }
  block: { // expect: no-labels
    if (!found) break block;
    use(found);
  }
  outer: while (true) { // expect: no-labels
    continue outer;
  }
  return found;
}

// Compliant: no labels.
for (const row of rows) {
  if (row) break;
}
const obj = { label: 1 };
switch (x) {
  case 1:
    break;
}

class Misparsed {
  method() {
    context: CanvasRenderingContext2D;
  }
}
const make = () => { value: 1 }; // expect: no-labels
