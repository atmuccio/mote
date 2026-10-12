function sum(rows, n) {
  let total = 0;
  for (let i = 0, j = 0; i < n; j++) { // expect: for-loop-counter-misuse
    total += rows[i];
  }
  for (let i = 0; i < rows.length; k++) { // expect: for-loop-counter-misuse
    total += rows[i];
  }
  for (let i = 0; i < n; i++) {
    if (rows[i] === '--') {
      i = n; // expect: for-loop-counter-misuse
    }
  }
  for (let i = 0; i < rows.length; i += 1) {
    const end = rows.indexOf(';', i);
    total += end;
    i = end; // expect: for-loop-counter-misuse
  }
  return total;
}

function compliant(rows, n, list, head) {
  for (let i = 0; i < n; i++) {
    if (rows[i] === '#') i++;
    if (rows[i] === '"') i += 2;
    if (rows[i] === '!') i = i + 1;
    if (rows[i] === '?') i = 1 + i;
    if (rows[i] === '<') i = i - 1;
  }
  for (let i = 0; i < list.length; i++) {
    if (list[i] == null) {
      list.splice(i, 1);
      i--;
    }
  }
  for (let node = head; node; node = node.next) {}
  for (let i = 0; node; i++) {
    node = node.next;
  }
  for (let i = 0; queue.length; i++) {
    queue.shift();
  }
  for (let i = 0; i < limit(); j++) {}
  for (let i = 0; i < this.size; j++) {}
  for (let i = 0; i < n; j++) {
    i = next(i);
  }
  for (let i = 0; i < pending.length; count++) {
    drain(pending);
  }
  for (let i = 0; i < n; i++) {
    const later = () => {
      i = 0;
    };
    later();
  }
  for (let i = 0; i < n; i++) {
    {
      let i = 5;
      i = 6;
    }
  }
  for (;;) {}
  for (let i = 0; i < n;) {
    i = step(i);
  }
  for (const x of rows) {}
  for (let i = 0; i < n; i++, advance()) {}
}
