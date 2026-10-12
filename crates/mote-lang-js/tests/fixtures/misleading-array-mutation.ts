export function positives(names: string[], data: object) {
  const sorted = names.sort(); // expect: misleading-array-mutation
  render(names, sorted);

  const letters = ['b', 'a', 'c'];
  const backwards = letters.reverse(); // expect: misleading-array-mutation
  render(letters, backwards);

  const keys = Object.keys(data);
  let ordered: string[] = [];
  ordered = keys.sort((a, b) => a.localeCompare(b)); // expect: misleading-array-mutation
  render(keys, ordered);

  const zeros = new Array<number>(3);
  const filled = zeros.fill(0); // expect: misleading-array-mutation
  render(zeros.map(String), filled.map(String));
}

class Holder {
  items: number[] = [];
  top: number[] = [];

  update(values: ReadonlyArray<number>, list: Array<number>) {
    this.top = list.sort(); // expect: misleading-array-mutation
    return [values, list[0]];
  }
}

// ---- compliant ----
export function negatives(names: string[], other: string[], anything: any, data: object) {
  names.sort();
  render(names, []);

  const sorted1 = [...names].sort();
  const sorted2 = names.slice().sort();
  const sorted3 = names.toSorted();
  render(names, sorted1.concat(sorted2, sorted3));

  // The original is not used again.
  const byLength = other.sort((a, b) => a.length - b.length);
  render(byLength, []);

  // Only its length is read again, which sorting does not change.
  const local = ['x', 'y'];
  const reordered = local.reverse();
  render(reordered, [String(local.length)]);

  // Used again only in ways that do not depend on the order.
  const parts: string[] = ['b', 'a'];
  const sortedParts = parts.sort();
  const all = [...parts, ...other].sort();
  render(sortedParts, all.concat(String(parts.includes('a'))));

  // Stored back into the same variable.
  let list = ['b', 'a'];
  list = list.sort();
  render(list, []);

  // Unknown receivers: may be an immutable collection whose sort returns a copy.
  const result = anything.sort();
  render(anything, result);

  // splice returns the removed elements.
  const removed = names.splice(0, 1);
  render(names, removed);

  // Returned: the caller decides.
  return Object.keys(data).sort();
}

function render(a: string[], b: string[]) {
  return a.length + b.length;
}

new Holder().update([], []);
