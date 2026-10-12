export function positives(options: Record<string, string>, list: string[], text: string, shell: string) {
  for (const key in Object.keys(options)) { // expect: no-for-in-array
    use(options[key]);
  }
  for (const i in [1, 2, 3]) { // expect: no-for-in-array
    use(i);
  }
  for (const other in list) { // expect: no-for-in-array
    if (other === shell) {
      continue;
    }
  }
  const pairs = Object.entries(options);
  for (const i in pairs) { // expect: no-for-in-array
    use(pairs[i], i);
  }
  const copy = Array.from(text);
  for (const i in copy) { // expect: no-for-in-array
    use(copy[Number(i) + 1]);
  }
  let k;
  for (k in new Array(3)) { // expect: no-for-in-array
    use(k);
  }
}

// ---- compliant ----
export function negatives(options: object, anything: any, record: Record<string, number>, list: string[]) {
  for (const key of Object.keys(options)) {
    use(key);
  }
  for (const key in options) {
    use(key);
  }
  for (const key in anything) {
    use(key);
  }
  for (const key in record) {
    use(key);
  }
  let reassigned = [1, 2];
  reassigned = anything;
  for (const key in reassigned) {
    use(key);
  }
  // The key only indexes the same array: works despite the string keys.
  for (const i in list) {
    use(list[i]);
  }
  let count = 0;
  for (const unused in list) {
    count++;
  }
  return count;
}

function use(...values: unknown[]) {
  return values;
}
