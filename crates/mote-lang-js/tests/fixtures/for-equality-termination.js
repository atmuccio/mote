export function positives(text, items) {
  for (let i = 0; i !== text.length; i++) { // expect: for-equality-termination
    if (text[i] === '\\') i++;
  }
  for (let i = 0; i != 9; i += 2) {} // expect: for-equality-termination
  for (let x = 0; x != 1; x += 0.1) {} // expect: for-equality-termination
  let j;
  for (j = 10; j !== 1; j = j - 2) {} // expect: for-equality-termination
  for (let k = 0; items.length !== k; k++) { // expect: for-equality-termination
    k += items[k];
  }
  for (let m = 0; m != 7; m = m + 3) {} // expect: for-equality-termination
}

// ---- compliant ----
export function negatives(text, n, step, head) {
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '\\') i++;
  }
  for (let i = 0; i != n; i++) {}
  for (let i = n; i !== 0; i--) {}
  for (let i = 0; i != 10; i += 2) {}
  for (let i = 0; i != n; i += 2) {} // the bound is not visible
  for (let i = 0; i != n; i += step) {}
  for (let node = head; node != null; node = node.next) {}
  for (let i = 0; i != n; i++) {
    const inner = () => {
      i++; // in a callback, not the loop's own flow
    };
    inner();
  }
  for (let i = 0; i != n; i++) {
    i = n - 1; // a jump, not a step
  }
  for (let i = 0; i < 9; i += 2) {}
}
