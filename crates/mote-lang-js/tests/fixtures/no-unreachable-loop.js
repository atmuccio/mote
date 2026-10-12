export function find(items, id) {
  let found;
  for (let i = 0; i < items.length; i++) { // expect: no-unreachable-loop
    if (items[i].id === id) {
      found = items[i];
    }
    break;
  }
  while (found) { // expect: no-unreachable-loop
    process(found);
    return found;
  }
  do { // expect: no-unreachable-loop
    step();
    throw new Error('stop');
  } while (running());
  outer: for (;;) { // expect: no-unreachable-loop
    for (const x of items) {
      if (x) continue;
    }
    break outer;
  }
  for (;;) { // expect: no-unreachable-loop
    if (found) {
      return 1;
    } else {
      return 2;
    }
  }
}

// Compliant.
export function ok(items, id) {
  for (let i = 0; i < items.length; i++) {
    if (items[i].id === id) {
      return items[i];
    }
  }
  for (const item of items) {
    return item;
  }
  for (const key in items) {
    return false;
  }
  while (true) {
    const next = read();
    if (!next) continue;
    return next;
  }
}

export function more(items) {
  do {
    step();
    break;
  } while (false);
  for (let i = 0; i < 3; i++) {
    try {
      return attempt(i);
    } catch (e) {
      log(e);
    }
  }
  for (let i = 0; i < 3; i++) {
    switch (items[i]) {
      case 1:
        break;
      default:
        return 2;
    }
  }
  loop: while (items.length) {
    for (const x of items) {
      if (x) continue loop;
    }
    break;
  }
  for (let i = 0; i < 3; i++) {
    const cb = () => {
      return i;
    };
    cb();
  }
  for (let d = 3; d >= 0; d--) {
    check(d);
    // Remove the next line to run every case.
    break;
  }
  while (items.length) {
    items.pop();
    break; // one pass is enough for now
  }
  return 0;
}
